/**
 * Canonical surface stream port (plan Phase B,
 * .agents/plans/workbench-intent-closure.md §4; REQ-SYNC-001,
 * REQ-DATA-002/003/004/007, REQ-SAFE-005, REQ-VERIFY-006).
 *
 * Binds `AuthoritativeStreamPort` (live-streams.ts, T16) to the REAL §20.2
 * HTTP routes (`GauntletReadClient`) instead of an in-process fake. Every
 * declaration is derived from an observed fact, never minted (D1):
 *
 *   - source identity  — the configured endpoint's run id from `GET /state`.
 *   - store epoch      — `GET /capabilities`' `protocol_version` +
 *     `server_version`, the current run id, and the observed unit-id set,
 *     joined into one opaque string. A server restart, a run change, or a
 *     change in which units exist replaces the epoch, which forces every
 *     `LiveStreamClient` tracking this source to take a fresh baseline
 *     (REQ-SYNC-001) — the same correctness property the deleted
 *     `WorkbenchHost` membership-diff gave by accident, now given by the
 *     port's own typed contract instead of ad hoc string comparison.
 *   - cursor semantics — an opaque sequence minted only by this port over
 *     observations it has itself folded; never compared lexically by callers
 *     (delegated to `InMemoryStreamSource`, the same reference primitive T16
 *     already proved correct for retention/epoch/cursor bookkeeping — D2).
 *   - retention        — connect-scoped: this port only ever retains events
 *     since its own construction (or since its last epoch replacement). A
 *     cursor from before that validates `retention_expired`, honestly.
 *   - granularity       — `"snapshot_interval"` (see `SourceGranularity` in
 *     live-streams.ts): this surface exposes periodic polled snapshots, not a
 *     durable per-event log. Two real changes to the same field between two
 *     observations fold into one delivered event. That is a declared
 *     property of this port, not a bug — a durable backend-ordered stream is
 *     a separate, unbuilt capability (workbench-intent-closure.md §6).
 *
 * The ONLY async step in this module is `observe()`: one real HTTP round trip
 * (`GET /state`, `GET /capabilities`, `GET /units/{id}/inspection` per unit)
 * that diffs the freshly fetched canonical content against the last
 * observation this port made and folds any difference into its internal
 * buffer. Everything `AuthoritativeStreamPort` requires
 * (`describeSources`/`baselineWindow`/`incrementsSince`) stays synchronous,
 * serving strictly from that buffer — so `LiveStreamClient`'s reconciliation
 * (T16, already proven against `InMemoryStreamPort`) needs no changes at all
 * to consume a real surface instead of an in-process fake.
 */

import type { CapabilitiesResponse, GauntletReadClient } from "./client";
import {
  type AuthoritativeStreamPort,
  InMemoryStreamPort,
  InMemoryStreamSource,
  type SourceDescriptor,
  type SourceWindow,
} from "./live-streams";

export const CANONICAL_SOURCE_ID = "gauntlet";

/** What one `observe()` round trip found. */
export interface ObservationReport {
  readonly reachable: boolean;
  readonly run_id: string | null;
  /** True when this observation folded a new event into the buffer. */
  readonly changed: boolean;
  /** True when at least one unit's inspection fetch failed this round
   *  (the unit itself is still reported; only its detail view is missing). */
  readonly degraded: boolean;
}

/**
 * `AuthoritativeStreamPort` over the real Gauntlet operator HTTP surface.
 * One instance owns one `InMemoryStreamSource` as its buffer — reusing T16's
 * proven retention/epoch/cursor bookkeeping rather than reimplementing it
 * (D2) — and repopulates that buffer from real HTTP responses on `observe()`.
 */
export class CanonicalSurfaceStreamPort implements AuthoritativeStreamPort {
  private readonly client: GauntletReadClient;
  private readonly source: InMemoryStreamSource;
  private readonly backing: InMemoryStreamPort;
  private epoch: string | null = null;
  private contentDigest = "";
  private reachable = true;
  private runId: string | null = null;
  private capabilities: CapabilitiesResponse | null = null;

  constructor(client: GauntletReadClient) {
    this.client = client;
    this.source = new InMemoryStreamSource(CANONICAL_SOURCE_ID, "epoch-unobserved");
    this.backing = new InMemoryStreamPort([this.source]);
  }

  /** The run id as of the last `observe()` (null before the first observe). */
  get lastRunId(): string | null {
    return this.runId;
  }

  /** Whether the surface answered the last `observe()` at all. */
  get lastReachable(): boolean {
    return this.reachable;
  }

  /** The capabilities handshake fetched by the last `observe()` (one HTTP
   *  round trip shared with epoch derivation — never fetched twice). */
  get lastCapabilities(): CapabilitiesResponse | null {
    return this.capabilities;
  }

  describeSources(): readonly SourceDescriptor[] {
    return this.backing.describeSources().map(withSnapshotGranularity);
  }

  baselineWindow(sourceId: string): SourceWindow {
    return withGranularWindow(this.backing.baselineWindow(sourceId));
  }

  incrementsSince(sourceId: string, afterSequence: number): SourceWindow {
    return withGranularWindow(this.backing.incrementsSince(sourceId, afterSequence));
  }

  /**
   * The one async step. Fetches state + capabilities + per-unit inspections,
   * derives the epoch from observed facts, and folds any content change into
   * the internal buffer as `StreamEvent`s — the same diff the deleted
   * `WorkbenchHost` hand-rolled, now flowing through the typed port contract.
   * Must be called before `LiveStreamClient.connect()`/`poll()` for either to
   * see fresh state; both of those stay synchronous.
   */
  async observe(): Promise<ObservationReport> {
    const [state, capabilities] = await Promise.all([
      this.client.fetchState(),
      this.client.fetchCapabilities(),
    ]);
    this.runId = state.run;
    this.capabilities = capabilities;
    if (state.status === "disconnected") {
      this.reachable = false;
      return { reachable: false, run_id: this.runId, changed: false, degraded: false };
    }
    this.reachable = true;

    const sortedUnitIds = state.units.map((unit) => unit.unit_id).sort();
    const epoch = JSON.stringify([
      capabilities?.protocol_version ?? "unobserved",
      capabilities?.server_version ?? "unobserved",
      state.run,
      sortedUnitIds,
    ]);
    if (epoch !== this.epoch) {
      // A server restart, a run change, or a membership change is a change of
      // identity for what this source IS, not an increment of it — replace
      // storage so the next reconciliation takes a fresh baseline
      // (REQ-SYNC-001), exactly as the deleted membership-diff did.
      this.source.replaceStorage(epoch);
      this.epoch = epoch;
      this.contentDigest = "";
    }

    const inspections = await Promise.all(
      state.units.map((unit) => this.client.fetchInspection(unit.unit_id, state.run ?? undefined)),
    );
    const degraded = inspections.some((value) => value === null);
    const content = JSON.stringify([
      state.units,
      inspections.map((value) => (value === null ? null : { ...value, fetched_at: "" })),
    ]);
    if (content === this.contentDigest) {
      return { reachable: true, run_id: this.runId, changed: false, degraded };
    }
    this.contentDigest = content;

    const timestamp_ms = Date.now();
    state.units.forEach((unit, index) => {
      const inspection = inspections[index];
      this.source.append({
        event_type: "unit_updated",
        timestamp_ms,
        payload: {
          unit_id: unit.unit_id,
          title: inspection?.claim ?? unit.unit_id,
          phase: unit.tier ?? inspection?.tier ?? "unknown",
          status: unit.status,
          authority: inspection?.authority ?? "unknown",
          blast_radius: inspection?.blast_radius ?? "unknown",
        },
      });
      for (const verification of inspection?.verifications ?? []) {
        if (!verification.evidence) continue;
        this.source.append({
          event_type: "evidence_added",
          timestamp_ms,
          payload: {
            evidence_id: verification.evidence,
            unit_id: unit.unit_id,
            kind: "verification",
            locator: verification.evidence,
            verified: verification.outcome === "pass",
          },
        });
      }
    });
    return { reachable: true, run_id: this.runId, changed: true, degraded };
  }
}

function withSnapshotGranularity(descriptor: SourceDescriptor): SourceDescriptor {
  return { ...descriptor, granularity: "snapshot_interval" };
}

function withGranularWindow(window: SourceWindow): SourceWindow {
  return { ...window, descriptor: withSnapshotGranularity(window.descriptor) };
}
