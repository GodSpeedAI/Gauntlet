import { Database } from "bun:sqlite";

/**
 * Reconstructible Bun SQLite projection store with generations and cursors.
 * Conforms to spec §20.2, REQ-DATA-002..004, REQ-DATA-007, REQ-BUN-003, REQ-STATE-002, and plan T06.
 *
 * Core Invariants:
 *   - Local SQLite holds only rebuildable read-model projections; authoritative state remains
 *     strictly with Gauntlet, Prime, or DomainForge (Attack 1 defense).
 *   - Frames from a superseded generation MUST NOT update the active Workbench Model (Attack 2 defense).
 *   - Applying one authoritative event atomically updates its local cursor and all dependent
 *     SQLite read-model rows in a single coherent transaction (REQ-DATA-004).
 *   - Local projection storage preserves enough provenance to explain which owning source
 *     and cursor produced each projected record (REQ-DATA-007).
 */

export type GenerationStatus = "syncing" | "ready" | "degraded" | "closed";

export interface ProjectionGeneration {
  readonly id: string;
  status: GenerationStatus;
  readonly started_at: number;
  closed_at?: number | undefined;
  readonly source_cursors: Map<string, string>;
}

export interface AuthoritativeEventFrame {
  readonly generation_id: string;
  readonly source_id: string;
  readonly cursor: string;
  readonly event_type: "unit_updated" | "evidence_added" | "session_updated" | "focus_changed";
  readonly payload: Record<string, unknown>;
  readonly timestamp_ms: number;
}

export interface UnitProjection {
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

export interface EvidenceProjection {
  readonly evidence_id: string;
  readonly generation_id: string;
  readonly source_id: string;
  readonly source_cursor: string;
  readonly unit_id: string;
  readonly kind: string;
  readonly locator: string;
  readonly verified: boolean;
  readonly updated_at: number;
}

export interface SessionProjection {
  readonly session_id: string;
  readonly generation_id: string;
  readonly source_id: string;
  readonly source_cursor: string;
  readonly parent_id: string | null;
  readonly model: string;
  readonly status: string;
  readonly updated_at: number;
}

export interface WorkbenchSnapshot {
  readonly id: string;
  readonly generation: string;
  readonly horizon: string | null;
  readonly focus: { kind: string; id: string } | null;
  readonly source_cursors: Record<string, string>;
  readonly affordance_ids: readonly string[];
  readonly units: readonly UnitProjection[];
  readonly sessions: readonly SessionProjection[];
  readonly evidence: readonly EvidenceProjection[];
}

export class SqliteProjectionStore {
  readonly dbPath: string;
  private db: Database;
  private activeGeneration: ProjectionGeneration | null = null;

  constructor(dbPath = ":memory:") {
    this.dbPath = dbPath;
    this.db = new Database(dbPath);
    this.initSchema();
  }

  /**
   * Initialize SQLite projection tables with provenance columns.
   */
  private initSchema(): void {
    this.db.run("PRAGMA journal_mode = WAL;");
    this.db.run("PRAGMA synchronous = NORMAL;");

    this.db.run(`
      CREATE TABLE IF NOT EXISTS generations (
        id TEXT PRIMARY KEY,
        status TEXT NOT NULL,
        started_at INTEGER NOT NULL,
        closed_at INTEGER
      );
    `);

    this.db.run(`
      CREATE TABLE IF NOT EXISTS source_cursors (
        generation_id TEXT NOT NULL,
        source_id TEXT NOT NULL,
        cursor TEXT NOT NULL,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (generation_id, source_id)
      );
    `);

    this.db.run(`
      CREATE TABLE IF NOT EXISTS units_projection (
        unit_id TEXT PRIMARY KEY,
        generation_id TEXT NOT NULL,
        source_id TEXT NOT NULL,
        source_cursor TEXT NOT NULL,
        title TEXT NOT NULL,
        phase TEXT NOT NULL,
        status TEXT NOT NULL,
        authority TEXT NOT NULL,
        blast_radius TEXT NOT NULL,
        updated_at INTEGER NOT NULL
      );
    `);

    this.db.run(`
      CREATE TABLE IF NOT EXISTS evidence_projection (
        evidence_id TEXT PRIMARY KEY,
        generation_id TEXT NOT NULL,
        source_id TEXT NOT NULL,
        source_cursor TEXT NOT NULL,
        unit_id TEXT NOT NULL,
        kind TEXT NOT NULL,
        locator TEXT NOT NULL,
        verified INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
      );
    `);

    this.db.run(`
      CREATE TABLE IF NOT EXISTS sessions_projection (
        session_id TEXT PRIMARY KEY,
        generation_id TEXT NOT NULL,
        source_id TEXT NOT NULL,
        source_cursor TEXT NOT NULL,
        parent_id TEXT,
        model TEXT NOT NULL,
        status TEXT NOT NULL,
        updated_at INTEGER NOT NULL
      );
    `);

    this.db.run(`
      CREATE TABLE IF NOT EXISTS focus_projection (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        generation_id TEXT NOT NULL,
        focus_kind TEXT NOT NULL,
        focus_id TEXT NOT NULL,
        updated_at INTEGER NOT NULL
      );
    `);
  }

  /**
   * Start a new ProjectionGeneration.
   * Closes any previous generation and updates active tracking.
   */
  startGeneration(id: string): ProjectionGeneration {
    if (this.activeGeneration) {
      this.closeGeneration(this.activeGeneration.id);
    }

    const now = Date.now();
    this.db.run("INSERT OR REPLACE INTO generations (id, status, started_at) VALUES (?, ?, ?);", [
      id,
      "syncing",
      now,
    ]);

    this.activeGeneration = {
      id,
      status: "syncing",
      started_at: now,
      source_cursors: new Map<string, string>(),
    };

    return { ...this.activeGeneration };
  }

  /**
   * Transition generation status to ready.
   */
  markGenerationReady(id: string): void {
    if (this.activeGeneration && this.activeGeneration.id === id) {
      this.activeGeneration.status = "ready";
      this.db.run("UPDATE generations SET status = ? WHERE id = ?;", ["ready", id]);
    }
  }

  /**
   * Close a generation.
   */
  closeGeneration(id: string): void {
    const now = Date.now();
    this.db.run("UPDATE generations SET status = ?, closed_at = ? WHERE id = ?;", [
      "closed",
      now,
      id,
    ]);
    if (this.activeGeneration && this.activeGeneration.id === id) {
      this.activeGeneration.status = "closed";
      this.activeGeneration.closed_at = now;
    }
  }

  /**
   * Get active generation.
   */
  getActiveGeneration(): ProjectionGeneration | null {
    if (!this.activeGeneration) return null;
    return {
      ...this.activeGeneration,
      source_cursors: new Map(this.activeGeneration.source_cursors),
    };
  }

  /**
   * Apply an authoritative event to the projection store.
   *
   * Invariant 1 (Attack 2 defense):
   *   Late frames from a superseded or mismatched generation MUST be rejected.
   *
   * Invariant 2 (REQ-DATA-004):
   *   Cursor advancement and dependent projection rows commit atomically in one transaction.
   */
  applyEvent(frame: AuthoritativeEventFrame): { applied: boolean; reason?: string } {
    if (!this.activeGeneration) {
      return { applied: false, reason: "no_active_generation" };
    }

    // ATTACK 2 DEFENSE: Check generation identity strictly
    if (frame.generation_id !== this.activeGeneration.id) {
      return {
        applied: false,
        reason: `superseded_generation: frame belongs to '${frame.generation_id}', but active generation is '${this.activeGeneration.id}'`,
      };
    }

    if (this.activeGeneration.status === "closed") {
      return { applied: false, reason: "generation_closed" };
    }

    // Atomic SQLite transaction
    const transaction = this.db.transaction(() => {
      // 1. Update source cursor
      this.db.run(
        `INSERT OR REPLACE INTO source_cursors (generation_id, source_id, cursor, updated_at)
         VALUES (?, ?, ?, ?);`,
        [frame.generation_id, frame.source_id, frame.cursor, frame.timestamp_ms],
      );

      // 2. Update projected entities based on event_type
      switch (frame.event_type) {
        case "unit_updated": {
          const p = frame.payload;
          this.db.run(
            `INSERT OR REPLACE INTO units_projection
             (unit_id, generation_id, source_id, source_cursor, title, phase, status, authority, blast_radius, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?);`,
            [
              String(p["unit_id"]),
              frame.generation_id,
              frame.source_id,
              frame.cursor,
              String(p["title"] ?? "Unit"),
              String(p["phase"] ?? "phase1"),
              String(p["status"] ?? "pending"),
              String(p["authority"] ?? "default"),
              String(p["blast_radius"] ?? "isolated"),
              frame.timestamp_ms,
            ],
          );
          break;
        }

        case "evidence_added": {
          const p = frame.payload;
          this.db.run(
            `INSERT OR REPLACE INTO evidence_projection
             (evidence_id, generation_id, source_id, source_cursor, unit_id, kind, locator, verified, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?);`,
            [
              String(p["evidence_id"]),
              frame.generation_id,
              frame.source_id,
              frame.cursor,
              String(p["unit_id"]),
              String(p["kind"] ?? "report"),
              String(p["locator"] ?? ""),
              p["verified"] ? 1 : 0,
              frame.timestamp_ms,
            ],
          );
          break;
        }

        case "session_updated": {
          const p = frame.payload;
          this.db.run(
            `INSERT OR REPLACE INTO sessions_projection
             (session_id, generation_id, source_id, source_cursor, parent_id, model, status, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?);`,
            [
              String(p["session_id"]),
              frame.generation_id,
              frame.source_id,
              frame.cursor,
              p["parent_id"] ? String(p["parent_id"]) : null,
              String(p["model"] ?? "default-model"),
              String(p["status"] ?? "running"),
              frame.timestamp_ms,
            ],
          );
          break;
        }

        case "focus_changed": {
          const p = frame.payload;
          this.db.run(
            `INSERT OR REPLACE INTO focus_projection (id, generation_id, focus_kind, focus_id, updated_at)
             VALUES (1, ?, ?, ?, ?);`,
            [frame.generation_id, String(p["kind"]), String(p["id"]), frame.timestamp_ms],
          );
          break;
        }
      }
    });

    transaction();

    // Update in-memory cursor map
    this.activeGeneration.source_cursors.set(frame.source_id, frame.cursor);

    return { applied: true };
  }

  /**
   * Produce an immutable snapshot from projected SQLite tables.
   */
  getSnapshot(horizon: string | null = null): WorkbenchSnapshot {
    if (!this.activeGeneration) {
      throw new Error("Cannot get snapshot without an active generation.");
    }

    const genId = this.activeGeneration.id;

    // Fetch cursors
    const cursorRows = this.db
      .prepare(
        "SELECT source_id, cursor FROM source_cursors WHERE generation_id = ? ORDER BY source_id ASC;",
      )
      .all(genId) as Array<{ source_id: string; cursor: string }>;

    const cursors: Record<string, string> = {};
    for (const r of cursorRows) {
      cursors[r.source_id] = r.cursor;
    }

    // Fetch units
    const unitRows = this.db
      .prepare("SELECT * FROM units_projection WHERE generation_id = ? ORDER BY unit_id ASC;")
      .all(genId) as UnitProjection[];

    // Fetch evidence
    interface EvidenceRow {
      evidence_id: string;
      generation_id: string;
      source_id: string;
      source_cursor: string;
      unit_id: string;
      kind: string;
      locator: string;
      verified: number;
      updated_at: number;
    }

    const evidenceRows = this.db
      .prepare(
        "SELECT * FROM evidence_projection WHERE generation_id = ? ORDER BY evidence_id ASC;",
      )
      .all(genId) as EvidenceRow[];

    const normalizedEvidence: EvidenceProjection[] = evidenceRows.map((e) => ({
      evidence_id: e.evidence_id,
      generation_id: e.generation_id,
      source_id: e.source_id,
      source_cursor: e.source_cursor,
      unit_id: e.unit_id,
      kind: e.kind,
      locator: e.locator,
      verified: Boolean(e.verified),
      updated_at: e.updated_at,
    }));

    // Fetch sessions
    const sessionRows = this.db
      .prepare("SELECT * FROM sessions_projection WHERE generation_id = ? ORDER BY session_id ASC;")
      .all(genId) as SessionProjection[];

    // Fetch focus
    const focusRow = this.db
      .prepare("SELECT focus_kind, focus_id FROM focus_projection WHERE generation_id = ?;")
      .get(genId) as { focus_kind: string; focus_id: string } | undefined;

    const focus = focusRow ? { kind: focusRow.focus_kind, id: focusRow.focus_id } : null;

    // Snapshot ID derived deterministically
    const snapshotId = `snap-${genId}-${Object.values(cursors).join(":") || "empty"}`;

    return {
      id: snapshotId,
      generation: genId,
      horizon,
      focus,
      source_cursors: cursors,
      affordance_ids: unitRows.map((u) => `affordance-${u.unit_id}`),
      units: unitRows,
      sessions: sessionRows,
      evidence: normalizedEvidence,
    };
  }

  /**
   * Close the SQLite connection cleanly.
   */
  close(): void {
    this.db.close();
  }

  /**
   * ATTACK 1 DEFENSE (Cold Rebuild):
   * Completely reconstruct the SQLite database from an authoritative event log.
   */
  static rebuildFromAuthoritativeLog(
    dbPath: string,
    generationId: string,
    events: readonly AuthoritativeEventFrame[],
  ): SqliteProjectionStore {
    const store = new SqliteProjectionStore(dbPath);
    store.startGeneration(generationId);

    for (const ev of events) {
      store.applyEvent(ev);
    }

    store.markGenerationReady(generationId);
    return store;
  }
}
