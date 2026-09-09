import { type AuthoritativeEventFrame, SqliteProjectionStore } from "./projection-store";

// Benchmark: representative replay volume folded into bun:sqlite projection store
const eventCount = 10_000;
const store = new SqliteProjectionStore(":memory:");
store.startGeneration("gen-bench");

const events: AuthoritativeEventFrame[] = [];
for (let i = 0; i < eventCount; i++) {
  if (i % 3 === 0) {
    events.push({
      generation_id: "gen-bench",
      source_id: "gauntlet",
      cursor: `g-cur-${i}`,
      event_type: "unit_updated",
      payload: {
        unit_id: `unit-${i % 200}`,
        title: `Unit Task ${i}`,
        phase: "execution",
        status: i % 2 === 0 ? "active" : "settled",
        authority: "normal",
        blast_radius: "isolated",
      },
      timestamp_ms: 1000 + i,
    });
  } else if (i % 3 === 1) {
    events.push({
      generation_id: "gen-bench",
      source_id: "gauntlet",
      cursor: `g-cur-${i}`,
      event_type: "evidence_added",
      payload: {
        evidence_id: `ev-${i}`,
        unit_id: `unit-${i % 200}`,
        kind: "execution_evidence",
        locator: `.agents/reports/ev-${i}.json`,
        verified: true,
      },
      timestamp_ms: 1000 + i,
    });
  } else {
    events.push({
      generation_id: "gen-bench",
      source_id: "prime",
      cursor: `p-cur-${i}`,
      event_type: "session_updated",
      payload: {
        session_id: `ses-${i % 50}`,
        parent_id: null,
        model: "claude-3-5-sonnet",
        status: "active",
      },
      timestamp_ms: 1000 + i,
    });
  }
}

console.log(`=== SQLite Replay Benchmark (${eventCount} Authoritative Events) ===`);

const startReplay = performance.now();
for (let i = 0; i < eventCount; i++) {
  const ev = events[i];
  if (ev) {
    store.applyEvent(ev);
  }
}
const replayDuration = performance.now() - startReplay;
const replayOps = Math.round((eventCount / replayDuration) * 1000);

console.log(
  `[SqliteProjectionStore] Replay ${eventCount} events: ${replayDuration.toFixed(2)}ms (${replayOps.toLocaleString()} events/sec)`,
);

const startSnapshot = performance.now();
const snapshot = store.getSnapshot("horizon-bench");
const snapshotDuration = performance.now() - startSnapshot;

console.log(
  `[SqliteProjectionStore] getSnapshot(): ${snapshotDuration.toFixed(2)}ms (units: ${snapshot.units.length}, sessions: ${snapshot.sessions.length}, evidence: ${snapshot.evidence.length})`,
);

console.log("=== SQLite Replay Benchmark Complete ===");
store.close();
