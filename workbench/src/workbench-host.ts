import { mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import {
  type AffordanceObservation,
  EMPTY_AFFORDANCE_OBSERVATION,
  resolveAffordance,
  statusPermitsInvocation,
} from "./affordance";
import { CanonicalSurfaceStreamPort } from "./canonical-surface-stream-port";
import { GauntletReadClient } from "./client";
import type { ComposerMode } from "./composer";
import type { WorkbenchConfig } from "./config";
import type { FocusRef } from "./focus";
import { LiveStreamClient } from "./live-streams";
import {
  HttpOperatorActionRoute,
  OperationGateway,
  type OperationSession,
} from "./operation-gateway";
import { plannedOperationForComposerMode } from "./planned-operations";
import { PrimeDelegationDispatcher } from "./prime-recursion";
import { RealPrimeHost } from "./real-prime-host";
import { interpretSxrRefreshOutcome } from "./reality-trace-binding";
import type { RealityTraceLane } from "./reality-trace-lane";
import type { ShellInteractions } from "./shell";
import { bunSxrProcessRunner, executeSxrRefresh, type SxrLensProjection } from "./sxr-adapter";
import { WorkbenchModel } from "./workbench-model";

/**
 * Isolated, workbench-owned Prime agent/session directories, derived from
 * the SAME root the projection store lives under (never the operator's real
 * `~/.prime` — plan Phase C2). `:memory:` projection stores (tests) fall
 * back to a `.gauntlet/` root under the current working directory.
 */
export function primeDirs(config: WorkbenchConfig): { agentDir: string; sessionDir: string } {
  // anchorPath (T13) requires absolute roots — a relative projection_store
  // (the DEFAULT_CONFIG shape, e.g. ".gauntlet/projections.db") must be
  // resolved against cwd, not passed through as-is (caught RED the first
  // time this used a bare relative path).
  const root =
    config.projection_store === ":memory:"
      ? resolve(".gauntlet")
      : resolve(dirname(config.projection_store));
  return { agentDir: `${root}/prime-agent`, sessionDir: `${root}/prime-sessions` };
}

/**
 * Production composition of read projections and the canonical operation
 * gateway. Read projection is bound over `CanonicalSurfaceStreamPort` +
 * `LiveStreamClient` (plan Phase B, workbench-intent-closure.md §4;
 * REQ-DATA-002/003/004/007, REQ-SYNC-001, REQ-SAFE-005, REQ-VERIFY-006) —
 * there is no hand-rolled diff here; every declaration the port makes
 * (source identity, store epoch, cursor, retention, granularity) is derived
 * from an observed HTTP fact and reconciled through T16's already-proven
 * generation/cursor machinery, the same machinery `InMemoryStreamPort`-backed
 * clients use in `cold-rebuild.test.ts`/`detach-reattach.test.ts`/etc.
 *
 * The gateway's pre-admission observation (`route.observe`, below) is a
 * SEPARATE HTTP round trip and never reads through `stream`/`port` — the
 * router-not-authority containment line (see `operation-isomorphism.test.ts`
 * "hard containment" suite).
 */
export class WorkbenchHost {
  readonly model: WorkbenchModel;
  observation: AffordanceObservation = EMPTY_AFFORDANCE_OBSERVATION;
  status: "connecting" | "connected" | "empty" | "disconnected" = "connecting";
  notice: string | null = null;
  runId: string | null = null;
  private readonly client: GauntletReadClient;
  private readonly route: HttpOperatorActionRoute;
  private readonly gateway: OperationGateway;
  private readonly session: OperationSession | null;
  private readonly port: CanonicalSurfaceStreamPort;
  private readonly stream: LiveStreamClient;
  /**
   * The Prime delegation dispatcher, bound to a REAL `RealPrimeHost` (plan
   * Phase C2) — `null` when `config.prime_binary` is unconfigured (D6: Prime
   * absence never degrades Gauntlet; every read/operation path above is
   * unaffected by this being null). Composer-level `prime`/`message`/`kernel`
   * submission routing through this dispatcher is a follow-on to this phase
   * (see `evidence/workbench-intent-closure/phase-c2/`); this field's
   * presence is what the gated real-substrate battery and the RED-first
   * teeth (denial-class zero-spawn, receipt-before-spawn, concurrent
   * same-key refusal, D6) exercise directly.
   */
  readonly primeDispatcher: PrimeDelegationDispatcher | null;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private changed: () => void = () => {};
  private closed = false;
  private pending: Promise<void> | null = null;
  /**
   * ICM §30 BUSY_SUBMISSION wiring: the `${operation}:${focus.id}` keys with
   * an admission round-trip currently in flight through `this.gateway.invoke`
   * (matches the `affordance:${operation}:${focus.id}` record-id convention
   * in inspector.ts/views-work.ts, minus the `affordance:` prefix since this
   * set is keyed by operation+focus alone, not by record kind). Never
   * includes `state.refresh`, which is a local read short-circuit, not a
   * gateway submission. Read-only to callers.
   */
  private readonly pendingOperationKeys = new Set<string>();

  get pendingOperations(): ReadonlySet<string> {
    return this.pendingOperationKeys;
  }

  /**
   * RealityTrace (SXR) refresh state (GUI-redesign increment 6, ICM §16.11:
   * refresh is command-triggered only, never during render). `sxrRefreshEpoch`
   * increments on every refresh attempt so a superseded in-flight refresh can
   * discard its result when it resolves after a newer one started.
   */
  private sxrRefreshEpoch = 0;
  private sxrLensState: SxrLensProjection | undefined;
  private sxrLaneState: RealityTraceLane | undefined;

  get sxrLens(): SxrLensProjection | undefined {
    return this.sxrLensState;
  }

  get sxrLane(): RealityTraceLane | undefined {
    return this.sxrLaneState;
  }

  get sxrRefreshEpochCurrent(): number {
    return this.sxrRefreshEpoch;
  }

  constructor(
    readonly config: WorkbenchConfig,
    private readonly options: { actor?: string; pollIntervalMs?: number } = {},
  ) {
    if (config.projection_store !== ":memory:")
      mkdirSync(dirname(config.projection_store), { recursive: true });
    this.model = new WorkbenchModel({ dbPath: config.projection_store, enableChat: false });
    // A placeholder generation so getSnapshot() is safe before the first real
    // connect; LiveStreamClient.connect() replaces it with a real one on the
    // first successful observation.
    this.model.startGeneration(crypto.randomUUID());
    this.client = new GauntletReadClient(config.gauntlet_endpoint);
    this.route = new HttpOperatorActionRoute(config.gauntlet_endpoint);
    this.gateway = new OperationGateway(this.route);
    this.port = new CanonicalSurfaceStreamPort(this.client);
    this.stream = new LiveStreamClient({ model: this.model, port: this.port });
    if (config.prime_binary !== null) {
      // RealPrimeHost's own constructor creates agentDir/sessionDir itself
      // (0700 for the agent dir) — no directory setup needed here.
      const { agentDir, sessionDir } = primeDirs(config);
      this.primeDispatcher = new PrimeDelegationDispatcher({
        adapter: new RealPrimeHost({ binary: config.prime_binary, agentDir, sessionDir }),
        canonical_route: this.route,
      });
    } else {
      this.primeDispatcher = null;
    }
    this.session =
      options.actor?.trim() && config.session_token
        ? { actor: options.actor.trim(), token: config.session_token }
        : null;
  }

  get sessionContext() {
    return { actor: this.session?.actor ?? "", authenticated: this.session !== null };
  }

  start(onChange: () => void): void {
    this.changed = onChange;
    const poll = async () => {
      await this.refresh();
      if (!this.closed) this.timer = setTimeout(poll, this.options.pollIntervalMs ?? 2000);
    };
    void poll();
  }

  refresh(): Promise<void> {
    if (this.closed) return Promise.resolve();
    if (this.pending !== null) return this.pending;
    this.pending = this.read().finally(() => {
      this.pending = null;
    });
    return this.pending;
  }

  private async read(): Promise<void> {
    try {
      const [report, observation] = await Promise.all([
        this.port.observe(),
        this.route.observe({ unit: null }),
      ]);
      if (this.closed) return;
      if (!report.reachable) throw new Error("state unavailable");

      const focusBefore = this.model.getSnapshot().focus;
      const runBefore = this.runId;
      const isFirstConnect = this.stream.state === "idle";
      const pollReport = isFirstConnect ? null : this.stream.poll();
      if (isFirstConnect) this.stream.connect();
      if (this.closed) return;

      this.runId = report.run_id;
      // A resync (first connect, or the port declaring a fresh epoch —
      // server restart, run change, or unit-membership change) is the ONLY
      // trigger for clearing Focus: it means the store's observable shape
      // just changed under us. An ordinary incremental poll never touches it
      // (workbench-intent-closure.md Phase B tooth 6 — this is the same
      // membership-change rule the deleted hand-rolled diff enforced, now
      // driven off the stream's own resync signal instead of a string diff).
      const resynced = isFirstConnect || pollReport?.resync_triggered !== null;
      if (
        resynced &&
        focusBefore !== null &&
        (runBefore !== this.runId ||
          !this.model.getUnits().some((unit) => unit.unit_id === focusBefore.source_id))
      ) {
        this.model.clearFocus();
      }

      const units = this.model.getUnits();
      this.status = this.runId || units.length > 0 ? ("connected" as const) : ("empty" as const);
      const gauntletCapabilities = (this.port.lastCapabilities?.capabilities ?? []).map(
        (capability) => ({
          capability,
          installed: true,
          gates: {
            visibility: false,
            reachability: false,
            payment: false,
            governance: false,
            settlement_access: false,
            recovery: false,
          },
        }),
      );
      this.observation = {
        ...observation,
        degraded: report.degraded,
        // "prime" is inventory-only (REQ-AFF-004): `installed` reflects
        // config presence, never reachability. This read loop never probes
        // the real binary (that cost belongs to preflight, not every poll),
        // so `reachability` stays honestly false — the SAME all-gates-false
        // convention every other capability above already uses — until a
        // real interaction proves otherwise. `installed: true, available:
        // false` is the correct, honest shape (plan Phase C2, D1/NONGOAL-004)
        // and this must never loosen into a claimed availability.
        capabilities:
          this.primeDispatcher !== null
            ? [
                ...gauntletCapabilities,
                {
                  capability: "prime",
                  installed: true,
                  gates: {
                    visibility: false,
                    reachability: false,
                    payment: false,
                    governance: false,
                    settlement_access: false,
                    recovery: false,
                  },
                },
              ]
            : gauntletCapabilities,
      };
    } catch {
      if (this.closed) return;
      this.status = "disconnected";
      this.observation = EMPTY_AFFORDANCE_OBSERVATION;
    }
    this.changed();
  }

  interactions(quit: () => void): ShellInteractions {
    return {
      quit,
      selectFocus: (ref) => {
        this.model.selectFocus(ref);
        this.changed();
      },
      hoist: (ref) => {
        this.model.hoistToFocus(ref);
        this.changed();
      },
      dehoist: () => {
        this.model.dehoist();
        this.changed();
      },
      invokeAffordance: (operation, ref) => {
        void this.invoke(operation, ref);
      },
      submitComposer: (mode, destination, draft) => {
        void this.submit(mode, destination, draft);
      },
      refreshRealityTrace: () => {
        void this.refreshRealityTrace();
      },
    };
  }

  async invoke(operation: string, focus: FocusRef, note = ""): Promise<void> {
    if (operation === "state.refresh") {
      // ICM §30 honesty (GUI redesign, closes the increment-1 residual): a
      // read may refresh locally, but ONLY when the resolver currently
      // permits it — the palette row renders VISIBLE_DISABLED
      // (not_action_admitted), and activating a disabled control must not
      // silently do the thing anyway. The reason surfaces as the notice.
      const resolved = resolveAffordance(operation, focus, this.observation, {
        session: this.sessionContext,
      });
      if (!statusPermitsInvocation(resolved.status)) {
        this.notice = `state.refresh: ${resolved.status} (${resolved.reason_codes.join(", ")})`;
        this.changed();
        return;
      }
      await this.refresh();
      return;
    }
    this.notice = `Submitting ${operation}…`;
    // ICM §30 BUSY_SUBMISSION (GUI redesign follow-on): mint the pending key
    // for this operation+focus BEFORE the admission round-trip, so the very
    // next render can show the control as busy — and clear it in `finally`
    // so it drops whether the outcome is accepted/refused/outcome_unknown or
    // the gateway call throws. Never a domain-success claim by itself.
    const pendingKey = `${operation}:${focus.id}`;
    this.pendingOperationKeys.add(pendingKey);
    this.changed();
    try {
      const outcome = await this.gateway.invoke({
        operation,
        focus,
        session: this.session,
        payload: { note, run: this.runId ?? undefined },
      });
      if (this.closed) return;
      // ICM §33 reason grammar (V5 fix): the refusal carries its reason CODES
      // through to the operator — the old flat notice dropped them, showing
      // only the pre-admission class token. The structured fields stay on the
      // outcome; the notice renders them.
      if (outcome.class === "accepted") {
        this.notice = `${operation}: accepted by Gauntlet`;
      } else if (outcome.class === "outcome_unknown") {
        this.notice = `${operation}: outcome unknown; do not resubmit (${outcome.idempotency_key})`;
      } else if (outcome.class === "refused") {
        const reasons =
          outcome.reason_codes.length > 0 ? ` (${outcome.reason_codes.join(", ")})` : "";
        this.notice = `${operation}: refused — ${outcome.code}${reasons}`;
      } else {
        this.notice = `${operation}: ${outcome.class} — ${outcome.code}`;
      }
      await this.refresh();
    } finally {
      this.pendingOperationKeys.delete(pendingKey);
      this.changed();
    }
  }

  async submit(mode: ComposerMode, destination: string, draft: string): Promise<void> {
    if (mode === "operation") {
      const focus = this.model.getSnapshot().focus;
      if (focus !== null) {
        await this.invoke(destination, focus, draft);
        return;
      }
      this.notice = "Select a Focus before submitting an operation.";
    } else {
      const planned = plannedOperationForComposerMode(mode);
      this.notice =
        planned !== null
          ? `${mode} → ${destination}: unsupported; '${planned.id}' is declared (planned, not yet bound — workbench-intent-closure.md Phase C2). Message was not sent.`
          : `${mode} → ${destination}: unsupported; no live submission adapter is connected. Message was not sent.`;
    }
    this.changed();
  }

  /**
   * RealityTrace (SXR) bounded on-demand refresh (GUI-redesign increment 6,
   * ICM §16.11: command-triggered only, never during render). Unbound config
   * = no-op — `SxrLensProjection`'s status vocabulary has no "unbound" case.
   * A superseded in-flight refresh discards its result on resolution.
   */
  async refreshRealityTrace(): Promise<void> {
    const binding = this.config.sxrBinding;
    if (binding === undefined) return;
    const epoch = ++this.sxrRefreshEpoch;
    const outcome = await executeSxrRefresh(
      {
        binary_path: binding.binaryPath,
        repo_id: binding.repoId,
        command: { kind: "ledger_export" },
      },
      bunSxrProcessRunner(),
    );
    if (this.closed || epoch !== this.sxrRefreshEpoch) return;
    const result = interpretSxrRefreshOutcome(outcome, binding.repoId, epoch);
    if (result.lens !== undefined) this.sxrLensState = result.lens;
    if (result.lane !== undefined) this.sxrLaneState = result.lane;
    this.changed();
  }

  close(): void {
    this.closed = true;
    if (this.timer !== null) clearTimeout(this.timer);
    this.model.close();
  }
}
