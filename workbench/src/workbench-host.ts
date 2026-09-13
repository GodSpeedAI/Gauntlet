import { mkdirSync, openSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
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
import { OPERATION_IDS } from "./operation-registry";
import { plannedOperationForComposerMode } from "./planned-operations";
import { constructBoundedContextEnvelope } from "./prime-context-envelope";
import { hasChangeListener, isExtendedPrimeHost, type PrimeChatSession } from "./prime-host";
import { PrimeDelegationDispatcher } from "./prime-recursion";
import { isPidAlive, RealPrimeHost } from "./real-prime-host";
import { interpretSxrRefreshOutcome } from "./reality-trace-binding";
import type { RealityTraceLane } from "./reality-trace-lane";
import { focusForPrimeChild } from "./relations";
import type { ShellInteractions } from "./shell";
import { bunSxrProcessRunner, executeSxrRefresh, type SxrLensProjection } from "./sxr-adapter";
import { type SpawnPathPolicy, validateSpawnRequest } from "./terminal-session";
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
  /**
   * The most recently launched (prime.launch) child session id (plan P1) —
   * `prime.message`/`prime.cancel`'s default target when the operator has
   * not typed an explicit destination. Read-only to callers.
   */
  lastChildSessionId: string | null = null;
  /**
   * The Prime child session currently selected in the Chat/Navigator surface
   * (plan P2) — distinct from `lastChildSessionId` (the most recently
   * LAUNCHED child) because an operator can select an OLDER session to
   * review/message it without that changing which child a bare launch
   * would replace. Both are updated together on select or launch so the
   * common case (select-then-message, or launch-then-message) needs no
   * extra operator action. Read-only to callers; use `selectChildSession`.
   */
  selectedChildSessionId: string | null = null;
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
    private readonly options: {
      actor?: string;
      pollIntervalMs?: number;
      /**
       * TEST-ONLY: bind `primeDispatcher` over this adapter instead of a real
       * `RealPrimeHost` (plan P1 unit tests use the in-memory
       * `PrimeHostAdapter` test double here so launch/message/cancel logic is
       * exercised without spawning a real process). Ignored unless
       * `config.prime_binary` is also set — this override never widens WHEN
       * Prime is considered configured, only WHAT adapter backs it.
       */
      primeHostOverride?: import("./prime-host").PrimeHostPort;
    } = {},
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
      const adapter =
        options.primeHostOverride ??
        new RealPrimeHost({
          binary: config.prime_binary,
          agentDir,
          sessionDir,
          projectRoot: config.project_root ?? undefined,
        });
      // Plan P2 "cheap change signal": when the bound adapter can push a
      // notification (RealPrimeHost, and the in-memory PrimeHostAdapter test
      // double both can), wire it straight to `this.changed()` so a mailbox
      // reply or a completed/stopped session redraws the TUI without waiting
      // for the next poll tick — no separate polling of the Prime host.
      if (hasChangeListener(adapter)) {
        adapter.setChangeListener(() => this.changed());
      }
      this.primeDispatcher = new PrimeDelegationDispatcher({
        adapter,
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

  /**
   * Chat-surface projection of Prime child sessions (plan P2): the
   * dispatcher's bookkeeping (`listSessions`) joined against its own
   * delegation records (`listRecords`) for the target unit each was admitted
   * against. `[]` when Prime is unconfigured OR genuinely has zero sessions
   * — callers distinguish those with `primeDispatcher === null` /
   * `config.prime_binary`, exactly like every other Prime-absence check in
   * this file.
   */
  listPrimeSessions(): readonly PrimeChatSession[] {
    if (this.primeDispatcher === null) return [];
    const targetUnitBySession = new Map(
      this.primeDispatcher.listRecords().map((record) => [record.session_id, record.target_unit]),
    );
    return this.primeDispatcher.primeHost.listSessions().map((session) => ({
      session_id: session.handle.session_id,
      target_unit: targetUnitBySession.get(session.handle.session_id) ?? null,
      status: session.handle.status,
      admitted_at: session.handle.admitted_at,
      model: session.handle.model,
      messages: session.messages,
    }));
  }

  /**
   * `selectChildSession` (plan P2): sets both the current Focus (so
   * Navigator/Chat rows highlight the same identity through the ordinary
   * FocusRef mechanism — REQ-FOCUS-001, no parallel selection model) and the
   * default `prime.message`/`prime.cancel` target. The SAME effect happens
   * when the operator reaches a PrimeSession row through the generic
   * `select_focus` ShellEffect (Navigator keyboard Enter, Navigator/Chat
   * mouse click) — see `interactions().selectFocus` below; this method is
   * the direct, host-level entry point tests and other callers can use
   * without going through the shell reducer.
   */
  selectChildSession(sessionId: string): void {
    this.applySelectedChildSession(sessionId);
    this.model.selectFocus(focusForPrimeChild(sessionId));
    this.changed();
  }

  private applySelectedChildSession(sessionId: string): void {
    this.selectedChildSessionId = sessionId;
    this.lastChildSessionId = sessionId;
  }

  interactions(quit: () => void): ShellInteractions {
    return {
      quit,
      selectFocus: (ref) => {
        this.model.selectFocus(ref);
        if (ref.kind === "PrimeSession") {
          this.applySelectedChildSession(ref.source_id);
        }
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
      cancelPrimeChild: () => {
        void this.cancelPrimeChild();
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
    } else if (mode === "prime") {
      await this.launchPrimeChild(draft);
      return;
    } else if (mode === "message") {
      await this.messagePrimeChild(destination, draft);
      return;
    } else if (mode === "run") {
      await this.startRun(draft);
      return;
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
   * `prime.launch` (plan workbench-agent-interaction.md P1): the composer's
   * `prime` mode draft is the goal; the target is the current Focus, which
   * MUST be a unit. Admission is chosen from whatever the affordance resolver
   * currently reports available for that unit — `unit.resolve` preferred
   * (an open escalation ticket), else `unit.retry`; neither available refuses
   * with the exact resolver reason codes and spawns nothing. The notice is
   * ALWAYS the admission receipt (child session id) — never a completion
   * claim (REQ-PRIME-003).
   */
  private async launchPrimeChild(goal: string): Promise<void> {
    if (this.primeDispatcher === null) {
      this.notice = "prime.launch: Prime is not configured (set GAUNTLET_WORKBENCH_PRIME_BINARY).";
      this.changed();
      return;
    }
    const trimmedGoal = goal.trim();
    if (trimmedGoal.length === 0) {
      this.notice = "prime.launch: a goal is required. Message was not sent.";
      this.changed();
      return;
    }
    const focus = this.model.getSnapshot().focus;
    if (focus === null || focus.kind !== "WorkItem") {
      this.notice =
        "prime.launch: select a unit Focus before launching Prime. Message was not sent.";
      this.changed();
      return;
    }

    // Canonical admission for the focused unit precedes spawn (C2 design):
    // prefer unit.resolve (an open escalation ticket), else unit.retry;
    // neither available is a zero-spawn refusal with the exact reasons.
    const resolveDescriptor = resolveAffordance(
      OPERATION_IDS.UNIT_RESOLVE,
      focus,
      this.observation,
      {
        session: this.sessionContext,
      },
    );
    const retryDescriptor = resolveAffordance(OPERATION_IDS.UNIT_RETRY, focus, this.observation, {
      session: this.sessionContext,
    });
    const admissionOperation = statusPermitsInvocation(resolveDescriptor.status)
      ? OPERATION_IDS.UNIT_RESOLVE
      : statusPermitsInvocation(retryDescriptor.status)
        ? OPERATION_IDS.UNIT_RETRY
        : null;
    if (admissionOperation === null) {
      const reasons = [
        `unit.resolve: ${resolveDescriptor.reason_codes.join(",") || resolveDescriptor.status}`,
        `unit.retry: ${retryDescriptor.reason_codes.join(",") || retryDescriptor.status}`,
      ].join("; ");
      this.notice = `prime.launch: refused — no canonical admission available for ${focus.source_id} (${reasons})`;
      this.changed();
      return;
    }

    let envelope: ReturnType<typeof constructBoundedContextEnvelope>;
    try {
      const { horizon, plan } = this.model.planForFocus(focus);
      envelope = constructBoundedContextEnvelope({
        horizon,
        plan,
        role_instructions: [trimmedGoal],
      });
    } catch (error) {
      this.notice = `prime.launch: envelope construction failed — ${error instanceof Error ? error.message : String(error)}`;
      this.changed();
      return;
    }

    const childSessionId = `prime-${focus.source_id}-${crypto.randomUUID()}`;
    // Work mode (plan P1): tools enabled at the real project root when
    // configured; unconfigured falls back to the adapter's isolated default
    // (documented in config.ts's `project_root` field) — never guessed.
    const workMode =
      this.config.project_root !== null
        ? { toolsEnabled: true, cwd: this.config.project_root }
        : undefined;

    this.notice = `prime.launch: admitting ${childSessionId}…`;
    this.changed();
    const outcome = await this.primeDispatcher.delegate({
      operation_request: {
        operation: admissionOperation,
        focus,
        session: this.session,
        payload: { note: trimmedGoal, run: this.runId ?? undefined },
      },
      envelope,
      child_session_id: childSessionId,
      parent_session_id: null,
      task: trimmedGoal,
      work_mode: workMode,
    });

    if (outcome.class === "admitted") {
      // The freshly launched child is the natural next message/cancel
      // target (plan P2) — but launch must NOT move the current Focus off
      // the unit (an operator launching a second child against the SAME
      // unit needs that unit still selected); `applySelectedChildSession`
      // updates only the message/cancel target, never Focus. An explicit
      // `selectChildSession` call (Navigator/Chat row selection) is the only
      // path that also moves Focus.
      this.applySelectedChildSession(childSessionId);
      this.notice = `prime.launch: admitted — child session ${childSessionId} (receipt only, not a completion claim)`;
    } else if (outcome.class === "refused_by_policy") {
      this.notice = `prime.launch: refused — ${outcome.code} (${outcome.detail})`;
    } else {
      const reasons =
        outcome.gateway.class === "refused" ? ` (${outcome.gateway.reason_codes.join(", ")})` : "";
      this.notice = `prime.launch: refused by canonical admission — ${outcome.gateway.class}${reasons}`;
    }
    await this.refresh();
    this.changed();
  }

  /**
   * `prime.message` (plan P1): destination names a child session id; empty or
   * the composer's default placeholder falls back to the most recently
   * launched child. Delivery is communication, never completion
   * (REQ-PRIME-005) — the notice never claims the child is done.
   */
  private async messagePrimeChild(destination: string, draft: string): Promise<void> {
    if (this.primeDispatcher === null) {
      this.notice = "prime.message: Prime is not configured (set GAUNTLET_WORKBENCH_PRIME_BINARY).";
      this.changed();
      return;
    }
    const message = draft.trim();
    if (message.length === 0) {
      this.notice = "prime.message: message text is required. Message was not sent.";
      this.changed();
      return;
    }
    const trimmedDestination = destination.trim();
    const target =
      trimmedDestination === "" || trimmedDestination === "workbench"
        ? (this.selectedChildSessionId ?? this.lastChildSessionId)
        : trimmedDestination;
    if (target === null) {
      this.notice =
        "prime.message: no child session to message yet — launch one first (prime mode).";
      this.changed();
      return;
    }
    const host = this.primeDispatcher.primeHost;
    if (!isExtendedPrimeHost(host)) {
      this.notice = "prime.message: the bound Prime host does not support message delivery.";
      this.changed();
      return;
    }
    const result = host.continueChild(target, {
      sender: this.session?.actor ?? "operator",
      message,
    });
    this.applySelectedChildSession(target);
    this.notice = result.delivered
      ? `prime.message: delivered to ${target} (${result.mode}) — communication, not completion`
      : `prime.message: not delivered to ${target} — ${result.detail}`;
    await this.refresh();
    this.changed();
  }

  /**
   * `prime.cancel` (plan P1): stops the selected/most recent child. Idempotent
   * on an already-terminal session (RealPrimeHost.stopChild's own guarantee).
   */
  async cancelPrimeChild(targetSessionId?: string): Promise<void> {
    if (this.primeDispatcher === null) {
      this.notice = "prime.cancel: Prime is not configured (set GAUNTLET_WORKBENCH_PRIME_BINARY).";
      this.changed();
      return;
    }
    const target = targetSessionId ?? this.selectedChildSessionId ?? this.lastChildSessionId;
    if (target === null) {
      this.notice = "prime.cancel: no child session to cancel.";
      this.changed();
      return;
    }
    const host = this.primeDispatcher.primeHost;
    if (!isExtendedPrimeHost(host)) {
      this.notice = "prime.cancel: the bound Prime host does not support cancellation.";
      this.changed();
      return;
    }
    const outcome = host.stopChild(target);
    this.notice = outcome.stopped
      ? `prime.cancel: stopped ${target} — ${outcome.detail}`
      : `prime.cancel: ${target} — ${outcome.detail}`;
    await this.refresh();
    this.changed();
  }

  /**
   * `run.start` (plan workbench-agent-interaction.md increment R, ICM
   * §13.1 BIND_EXISTING): the canonical path IS the `gauntlet run` CLI
   * (§20.2's HTTP surface has no run-start route). ICM §13.2 availability
   * ("inspect why unavailable") is answered by `gauntlet run --dry-run`
   * itself — a real preflight that writes nothing and exits non-zero with
   * an exact refusal reason when the run cannot start (e.g. a missing
   * REQUIRED control setting). A refusal there means zero further spawn.
   */
  private async startRun(draft: string): Promise<void> {
    const binary = this.config.gauntlet_binary;
    const projectRoot = this.config.project_root;
    if (binary === null || projectRoot === null) {
      this.notice =
        "run.start: Gauntlet is not configured (set GAUNTLET_WORKBENCH_GAUNTLET_BINARY and GAUNTLET_WORKBENCH_PROJECT_ROOT).";
      this.changed();
      return;
    }
    const policy: SpawnPathPolicy = Object.freeze({
      allowedProgramRoots: [dirname(binary)],
      allowedPathRoots: [projectRoot],
    });

    const intent = draft.trim();
    if (intent.length === 0) {
      this.notice = "run.start: an intent is required. Message was not sent.";
      this.changed();
      return;
    }

    this.notice = `run.start: preflight (dry-run) for "${intent}"…`;
    this.changed();

    let dryRunBound: ReturnType<typeof validateSpawnRequest>;
    try {
      dryRunBound = validateSpawnRequest(
        { program: binary, args: ["run", "--dry-run", intent], cwd: projectRoot },
        policy,
      );
    } catch (error) {
      this.notice = `run.start: refused at the T13 spawn boundary — ${error instanceof Error ? error.message : String(error)}`;
      this.changed();
      return;
    }
    let dryRunOut: string;
    let dryRunErr: string;
    let dryRunCode: number;
    try {
      const proc = Bun.spawn([dryRunBound.program, ...dryRunBound.args], {
        cwd: dryRunBound.cwd,
        env: { ...process.env },
        stdout: "pipe",
        stderr: "pipe",
      });
      const [out, err, code] = await Promise.all([
        new Response(proc.stdout as ReadableStream<Uint8Array>).text(),
        new Response(proc.stderr as ReadableStream<Uint8Array>).text(),
        proc.exited,
      ]);
      dryRunOut = out;
      dryRunErr = err;
      dryRunCode = code;
    } catch (error) {
      this.notice = `run.start: dry-run preflight failed to spawn — ${error instanceof Error ? error.message : String(error)}`;
      this.changed();
      return;
    }
    if (dryRunCode !== 0) {
      // ICM §13.2: the dry-run's own refusal text IS the exact reason
      // surface (e.g. a REQUIRED control setting's configuration failure) —
      // never re-derived or guessed here.
      const refusal =
        dryRunErr.trim().length > 0
          ? dryRunErr.trim()
          : dryRunOut.trim().length > 0
            ? dryRunOut.trim()
            : `dry-run exited ${dryRunCode}`;
      this.notice = `run.start: refused — ${refusal}`;
      this.changed();
      return;
    }

    // Dry-run is clean: bind + spawn the REAL run, DETACHED (REQ-PRIME-009:
    // closing the TUI must not kill admitted work), inheriting the
    // operator's own environment so their control settings (REQUIRED,
    // fail-closed) apply exactly as they would from a real shell.
    let runBound: ReturnType<typeof validateSpawnRequest>;
    try {
      runBound = validateSpawnRequest(
        { program: binary, args: ["run", intent], cwd: projectRoot },
        policy,
      );
    } catch (error) {
      this.notice = `run.start: refused at the T13 spawn boundary — ${error instanceof Error ? error.message : String(error)}`;
      this.changed();
      return;
    }
    const logDir = join(projectRoot, ".gauntlet", "workbench-runs");
    mkdirSync(logDir, { recursive: true });
    const logPath = join(logDir, `${Date.now()}.log`);
    const logFd = openSync(logPath, "a");
    let proc: ReturnType<typeof Bun.spawn>;
    try {
      proc = Bun.spawn([runBound.program, ...runBound.args], {
        cwd: runBound.cwd,
        env: { ...process.env },
        stdin: "ignore",
        stdout: logFd,
        stderr: logFd,
        // Detached so this process's own exit (or a future TUI close) never
        // signals the child — the ONLY thing that stops admitted work is an
        // explicit canonical stop/cancel or governing policy.
        detached: true,
      });
    } catch (error) {
      this.notice = `run.start: spawn failed — ${error instanceof Error ? error.message : String(error)}`;
      this.changed();
      return;
    }
    proc.unref();
    this.notice = `run.start: admitted (pid ${proc.pid}); watching for the run id…`;
    this.changed();
    void this.watchRunStart(proc.pid, logPath);
  }

  /**
   * Tail the run's own log file for its FIRST `run: run_id=<id>` line — the
   * admission identity, never a completion claim (REQ-PRIME-003's discipline
   * applied here too: "run <id> started" is honest, "run complete" is a
   * lie). Reading the log rather than the process's own stdout stream keeps
   * this independent of the spawning process's own lifetime (the run is
   * detached and must survive it). If the process exits before ever
   * printing a run id, its captured output (which is its stderr, honestly)
   * becomes the notice instead of a fabricated success.
   */
  private async watchRunStart(pid: number | null, logPath: string): Promise<void> {
    const deadlineMs = Date.now() + 15_000;
    while (Date.now() < deadlineMs) {
      if (this.closed) return;
      let content = "";
      try {
        content = readFileSync(logPath, "utf8");
      } catch {
        /* not flushed yet */
      }
      const runIdLine = content.split("\n").find((line) => line.startsWith("run: run_id="));
      if (runIdLine !== undefined) {
        const runId = runIdLine.slice("run: run_id=".length).trim();
        this.notice = `run.start: run ${runId} started (admission only — not a completion claim)`;
        this.changed();
        return;
      }
      if (pid === null || !isPidAlive(pid)) {
        const tail = content.trim();
        this.notice =
          tail.length > 0
            ? `run.start: process exited before printing a run id — ${tail.slice(-2000)}`
            : "run.start: process exited before printing a run id (no output captured)";
        this.changed();
        return;
      }
      await new Promise((resolveWait) => setTimeout(resolveWait, 150));
    }
    this.notice = `run.start: admitted (pid ${pid}) but no run id observed within timeout — see ${logPath}`;
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
