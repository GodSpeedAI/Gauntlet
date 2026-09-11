/**
 * T23 cross-surface route registry and matrix harness (plan T23; REQ-AFF-001/002/003,
 * REQ-OPS-002/004, REQ-SEC-002, REQ-SXR-002 direction, REQ-UX-003, REQ-VERIFY-003/004).
 *
 * This module is THE consolidated inventory of every invocation route the
 * workbench currently implements to a consequential operation, plus the harness
 * that drives each route against ONE canonical admission body so the frozen
 * representative matrix (cross-surface-equivalence.test.ts) can demand
 * convergence:
 *
 *   - Every route entry names its adapter seam, how to drive it, and which
 *     side-effect ledger observes it. No route entry carries business logic:
 *     driving code here is plumbing onto the landed modules (T09 gateway and
 *     transports, T11 kernel callables, T15 plugin projections, T10 Prime
 *     delegation, T12 shell effects), never a second admission.
 *   - Surfaces that LOOK like routes but expose no invocation are registered
 *     honestly as non-invocation surfaces (the T20 released artifact is a cold
 *     read-only host loop; the T00A client is read-only) rather than forced
 *     into the matrix.
 *   - The harness seeds one scenario stub (the single admission point, exactly
 *     as every real route terminates in the one Rust write path), the ONE host
 *     gateway shared by workbench projections, and each projection seam; the
 *     side-effect counters make "zero side effect" and "effects total, not
 *     per-route" measurable assertions.
 *
 * This is TEST SUPPORT. It is not production code and grants no authority.
 */

import {
  type AffordanceDescriptor,
  type AffordanceObservation,
  resolveAffordance,
} from "../affordance";
import { FocusRef } from "../focus";
import { createHorizon, planMaterialization } from "../horizon";
import { KernelAffordanceDirectory } from "../kernel-affordances";
import {
  type CanonicalActionCommand,
  type CanonicalOutcome,
  type GatewayOutcome,
  HttpOperatorActionRoute,
  OperationGateway,
  type OperationInvocationRequest,
} from "../operation-gateway";
import { lookupOperation, OPERATION_IDS } from "../operation-registry";
import { createShellViewRegistryCell, PluginCompositionManager } from "../plugins";
import {
  constructBoundedContextEnvelope,
  type PrimeContextEnvelope,
} from "../prime-context-envelope";
import { PrimeHostAdapter } from "../prime-host";
import { PrimeReplKernel } from "../prime-kernel";
import { PrimeDelegationDispatcher } from "../prime-recursion";
import {
  applyShellEffects,
  createShellState,
  handleKey,
  paletteEntries,
  type ShellEffect,
  type ShellInteractions,
  type ShellState,
} from "../shell";
import { defaultViewRegistry } from "../views-chat-trace";
import { WorkbenchModel } from "../workbench-model";
import { CanonicalSurfaceStub, type StubSideEffects } from "./canonical-surface-stub";
import { createSamplePlugin } from "./sample-plugin";

// --- The frozen scenario vocabulary (bound by the test's own freeze) ----------

/** The durable run every anchored scenario payload names (the canonical precondition anchor). */
export const T23_RUN = "01JT23CROSSSURFRUN1";
/** The scenario session credentials (surface auth binds actor and token). */
export const T23_TOKEN = "t23-cross-surface-token";
export const T23_ACTOR = "operator-t23";
/** The ALLOWED representative's target: a failed unit whose retry precondition genuinely holds. */
export const T23_RETRY_UNIT = "AK-923";
export const T23_RETRY_TICKET = "ESC-923";
/** The DENIED representative's target: a blocked unit whose escalation resolve is authority-gated. */
export const T23_DENY_UNIT = "AK-924";
export const T23_DENY_TICKET = "ESC-924";

// --- The route registry ---------------------------------------------------------

/** Every currently implemented invocation route to a consequential operation. */
export type MatrixRouteId =
  | "gateway-http"
  | "gateway-shim"
  | "direct-api"
  | "direct-cli"
  | "kernel-callable"
  | "plugin-projection"
  | "prime-delegation"
  | "shell-effect";

/** The workbench-side routes: they share THE host gateway and its key ledger. */
export const GATEWAY_FAMILY_ROUTES: readonly MatrixRouteId[] = Object.freeze([
  "gateway-http",
  "gateway-shim",
  "kernel-callable",
  "plugin-projection",
  "prime-delegation",
  "shell-effect",
]);

/** The raw surface shapes: direct calls past the workbench (they ARE the admission). */
export const DIRECT_FAMILY_ROUTES: readonly MatrixRouteId[] = Object.freeze([
  "direct-api",
  "direct-cli",
]);

export interface CrossSurfaceRouteEntry {
  readonly id: MatrixRouteId;
  readonly title: string;
  readonly owning_task: string;
  /** The module hosting the seam (static-inspection subject). */
  readonly seam_module: string;
  /** How the route is driven, ending at the ONE canonical admission. */
  readonly invoke_path: string;
  /** The side-effect ledger that observes this route's effects. */
  readonly side_effect_source: string;
  /** Whether the route's request shape carries the §REQ-OPS-002 idempotency identity. */
  readonly carries_idempotency_key: boolean;
  /** Whether the route can carry payment estimates (the payment-denial channel). */
  readonly carries_payment_estimates: boolean;
  /** Whether the route carries the session binding (the settlement-access channel). */
  readonly carries_session: boolean;
}

/**
 * The registry as it stands after T09/T10/T11/T12/T15 landed their seams.
 * Entries are facts about landed modules; the static-inspection suite pins the
 * graph they describe.
 */
export const ROUTE_REGISTRY: readonly CrossSurfaceRouteEntry[] = Object.freeze([
  Object.freeze({
    id: "gateway-http" as MatrixRouteId,
    title: "Workbench gateway over the HTTP surface transport",
    owning_task: "T09",
    seam_module: "src/operation-gateway.ts",
    invoke_path:
      "OperationGateway.invoke → HttpOperatorActionRoute (loopback `gauntlet serve` wire: GET /api/v1/state, POST /api/v1/units/{unit}/{action_kind})",
    side_effect_source: "the canonical surface's settlement/ticket/admission counters",
    carries_idempotency_key: true,
    carries_payment_estimates: true,
    carries_session: true,
  }),
  Object.freeze({
    id: "gateway-shim" as MatrixRouteId,
    title: "Workbench gateway over the in-process adapter shim",
    owning_task: "T09",
    seam_module: "src/testing/canonical-surface-stub.ts",
    invoke_path:
      "OperationGateway.invokeVia(asInProcessRoute) — the shim re-validates the session at its boundary, then performs",
    side_effect_source: "the canonical surface's settlement/ticket/admission counters",
    carries_idempotency_key: true,
    carries_payment_estimates: true,
    carries_session: true,
  }),
  Object.freeze({
    id: "direct-api" as MatrixRouteId,
    title: "Direct API-shape surface call (run-anchored action endpoint shape)",
    owning_task: "T09 (mirror)",
    seam_module: "src/testing/canonical-surface-stub.ts",
    invoke_path:
      "CanonicalSurfaceStub.perform(kind, unit, operator, note, run) — no workbench code in between; transport auth lives at the wire boundary, not the body",
    side_effect_source: "the canonical surface's settlement/ticket/admission counters",
    carries_idempotency_key: false,
    carries_payment_estimates: false,
    carries_session: false,
  }),
  Object.freeze({
    id: "direct-cli" as MatrixRouteId,
    title: "Direct CLI-shape surface call (ticket-anchored escalations action)",
    owning_task: "T09 (mirror)",
    seam_module: "src/testing/canonical-surface-stub.ts",
    invoke_path:
      "CanonicalSurfaceStub.performViaCli(kind, unit, operator, note, ticket) — the operator names the ticket; the ticket's own run anchors the action",
    side_effect_source: "the canonical surface's settlement/ticket/admission counters",
    carries_idempotency_key: false,
    carries_payment_estimates: false,
    carries_session: false,
  }),
  Object.freeze({
    id: "kernel-callable" as MatrixRouteId,
    title: "Prime kernel callable affordance",
    owning_task: "T11",
    seam_module: "src/kernel-affordances.ts",
    invoke_path:
      "KernelAffordanceDirectory.current().action(op).invoke → KernelAffordanceSet.invoke → OperationGateway.invoke (registry vocabulary; the request shape carries no payment estimates)",
    side_effect_source: "the canonical surface's settlement/ticket/admission counters",
    carries_idempotency_key: true,
    carries_payment_estimates: false,
    carries_session: true,
  }),
  Object.freeze({
    id: "plugin-projection" as MatrixRouteId,
    title: "Plugin affordance-projection presented operation (host-driven)",
    owning_task: "T15",
    seam_module: "src/plugins.ts",
    invoke_path:
      "PluginCompositionManager.affordanceProjections() → projectionDescriptor (the T09 resolver predicts) → the HOST drives OperationGateway.invoke with projection.operation (plugins hold no gateway)",
    side_effect_source: "the canonical surface's settlement/ticket/admission counters",
    carries_idempotency_key: true,
    carries_payment_estimates: true,
    carries_session: true,
  }),
  Object.freeze({
    id: "prime-delegation" as MatrixRouteId,
    title: "Prime delegation dispatcher (eligible Prime context, in-process adapter model)",
    owning_task: "T10",
    seam_module: "src/prime-recursion.ts",
    invoke_path:
      "PrimeDelegationDispatcher.delegate → its gateway's invokeVia(PrimeDelegationRoute) → canonical dispatch FIRST, then nonblocking child spawn behind acceptance",
    side_effect_source:
      "the canonical surface's counters PLUS the Prime adapter's session ledger (a child must never spawn behind a denial)",
    carries_idempotency_key: true,
    carries_payment_estimates: true,
    carries_session: true,
  }),
  Object.freeze({
    id: "shell-effect" as MatrixRouteId,
    title: "Shell palette affordance activation (T12 effects edge)",
    owning_task: "T12",
    seam_module: "src/shell.ts",
    invoke_path:
      "handleKey(palette Enter) → ShellEffect invoke_affordance → applyShellEffects → interactions.invokeAffordance → the host's OperationGateway.invoke (the shell holds no gateway)",
    side_effect_source: "the canonical surface's settlement/ticket/admission counters",
    carries_idempotency_key: true,
    carries_payment_estimates: true,
    carries_session: true,
  }),
]);

/** A surface that resembles a route but exposes no consequential invocation. */
export interface NonInvocationSurface {
  readonly id: "release-host-loop" | "read-client";
  readonly title: string;
  readonly owning_task: string;
  readonly seam_module: string;
  /** The honest reason this is NOT yet an invocation route in the matrix. */
  readonly not_an_invocation_route_because: string;
}

/**
 * Honest non-routes. The T20-released artifact counts as a surface in the
 * inventory, recorded as not-yet-an-invocation-route rather than forced.
 */
export const NON_INVOCATION_SURFACES: readonly NonInvocationSurface[] = Object.freeze([
  Object.freeze({
    id: "release-host-loop" as const,
    title: "T20-released standalone workbench artifact (PTY launch loop)",
    owning_task: "T20",
    seam_module: "src/release-manifest.ts",
    not_an_invocation_route_because:
      "the released artifact is a cold, read-only host loop: launch qualification renders the shell and quits it; the artifact exposes no invocation entry (no affordance activation is driven in the release form), so there is no consequential route to exercise — recorded, not forced",
  }),
  Object.freeze({
    id: "read-client" as const,
    title: "Read-only HTTP operator-surface client (with the T18 auth-probe POST)",
    owning_task: "T00A",
    seam_module: "src/client.ts",
    not_an_invocation_route_because:
      "the client's reads are observation-only; its single POST is the T18 security probe (mutateUnit) that exercises the wire's authentication boundary on the SAME canonical action endpoint the gateway route drives — a test instrument over an existing route, not an independent invocation route, and transport auth can never authenticate an unauthorized operation (REQ-SEC-002)",
  }),
]);

// --- Normalization (one comparable shape for every route's outcome) -------------

/**
 * One route's outcome normalized for matrix comparison. Gateway outcomes and
 * raw surface outcomes are different WIRE shapes; the canonical semantics live
 * in these fields. `denied_pre_admission` is true when the workbench refused
 * BEFORE canonical admission (the fresh-affordance gate); surface-shape denials
 * happen AT the write boundary, so they are false by construction while still
 * being before-side-effect.
 */
export interface NormalizedRouteOutcome {
  readonly route: MatrixRouteId;
  readonly kind: "accepted" | "refused" | "failed" | "outcome_unknown";
  readonly operation: string | null;
  readonly idempotency_key: string | null;
  /** The canonical refusal-vocabulary item this outcome preserves (null when none). */
  readonly canonical_code: string | null;
  readonly reason_codes: readonly string[];
  readonly denied_pre_admission: boolean;
  /** The accepted canonical facts, present only for accepted outcomes. */
  readonly accepted_facts: {
    readonly run_id: string;
    readonly unit_id: string;
    readonly tickets_resolved: readonly string[];
    readonly committed_now: boolean;
  } | null;
}

type AcceptedFacts = NonNullable<NormalizedRouteOutcome["accepted_facts"]>;

function freezeAcceptedFacts(outcome: {
  readonly run_id: string;
  readonly unit_id: string;
  readonly tickets_resolved: readonly string[];
  readonly committed_now: boolean;
}): AcceptedFacts {
  return Object.freeze({
    run_id: outcome.run_id,
    unit_id: outcome.unit_id,
    tickets_resolved: Object.freeze([...outcome.tickets_resolved]),
    committed_now: outcome.committed_now,
  });
}

/** Normalize ANY route's outcome into the comparable matrix shape. */
export function normalizeRouteOutcome(
  route: MatrixRouteId,
  outcome: GatewayOutcome | CanonicalOutcome,
): NormalizedRouteOutcome {
  // The two wire families carry different discriminants (`class` vs `kind`);
  // narrowing on the discriminant's presence keeps every access checked.
  if ("class" in outcome) {
    if (outcome.class === "accepted") {
      return Object.freeze({
        route,
        kind: "accepted" as const,
        operation: outcome.operation,
        idempotency_key: outcome.idempotency_key,
        canonical_code: null,
        reason_codes: Object.freeze([]),
        denied_pre_admission: false,
        accepted_facts: freezeAcceptedFacts(outcome),
      });
    }
    if (outcome.class === "refused") {
      // The pre-admission class rides `code` and the canonical reason items
      // ride `reason_codes`; post-admission refusals carry the canonical code
      // in both.
      const canonicalCode = outcome.reason_codes[0] ?? outcome.code;
      return Object.freeze({
        route,
        kind: "refused" as const,
        operation: outcome.operation,
        idempotency_key: outcome.idempotency_key,
        canonical_code: canonicalCode,
        reason_codes: Object.freeze([...outcome.reason_codes]),
        denied_pre_admission: outcome.stale,
        accepted_facts: null,
      });
    }
    if (outcome.class === "failed") {
      return Object.freeze({
        route,
        kind: "failed" as const,
        operation: outcome.operation,
        idempotency_key: outcome.idempotency_key,
        canonical_code: outcome.code,
        reason_codes: Object.freeze([outcome.code]),
        denied_pre_admission: false,
        accepted_facts: null,
      });
    }
    return Object.freeze({
      route,
      kind: "outcome_unknown" as const,
      operation: outcome.operation,
      idempotency_key: outcome.idempotency_key,
      canonical_code: null,
      reason_codes: Object.freeze([]),
      denied_pre_admission: false,
      accepted_facts: null,
    });
  }
  if (outcome.kind === "accepted") {
    return Object.freeze({
      route,
      kind: "accepted" as const,
      operation: null,
      idempotency_key: null,
      canonical_code: null,
      reason_codes: Object.freeze([]),
      denied_pre_admission: false,
      accepted_facts: freezeAcceptedFacts(outcome),
    });
  }
  if (outcome.kind === "refused") {
    return Object.freeze({
      route,
      kind: "refused" as const,
      operation: null,
      idempotency_key: null,
      canonical_code: outcome.code,
      reason_codes: Object.freeze([outcome.code]),
      // A surface-shape refusal happens AT the write boundary — before side
      // effect, but not before admission.
      denied_pre_admission: false,
      accepted_facts: null,
    });
  }
  return Object.freeze({
    route,
    kind: "failed" as const,
    operation: null,
    idempotency_key: null,
    canonical_code: outcome.code,
    reason_codes: Object.freeze([outcome.code]),
    denied_pre_admission: false,
    accepted_facts: null,
  });
}

// --- The harness -----------------------------------------------------------------

/** What one route drive produced, with the route-specific side instruments. */
export interface RouteDriveResult {
  readonly outcome: GatewayOutcome | CanonicalOutcome;
  /** Prime only: whether THIS drive admitted a delegation child. */
  readonly prime_child_spawned: boolean;
  /** Prime only: the adapter's total live session count after the drive. */
  readonly prime_sessions_total: number;
  /** Kernel only: whether the drive invalidated the wrapper (stale-denial projection consequence). */
  readonly kernel_wrapper_invalidated: boolean;
}

export interface HarnessOptions {
  readonly run_id?: string | undefined;
  readonly bound_actor?: string | undefined;
  readonly session_token?: string | undefined;
}

const SHELL_DIMS = { cols: 120, rows: 40 } as const;
const GEN = "gen-t23-matrix";

/**
 * One scenario harness: ONE canonical admission body (the stub), ONE host
 * gateway shared by the workbench projections, and every landed projection
 * seam wired onto them. Fresh harness per matrix scenario; never shared across
 * scenarios.
 */
export class CrossSurfaceHarness {
  readonly stub: CanonicalSurfaceStub;
  private readonly server: ReturnType<typeof Bun.serve>;
  private readonly httpRoute: HttpOperatorActionRoute;
  private readonly shimRoute: ReturnType<CanonicalSurfaceStub["asInProcessRoute"]>;
  private readonly gateway: OperationGateway;
  private readonly kernel: KernelAffordanceDirectory;
  private readonly plugins: PluginCompositionManager;
  private readonly prime: PrimeDelegationDispatcher;
  private readonly primeAdapter: PrimeHostAdapter;
  private readonly shellSnapshot: ReturnType<WorkbenchModel["getSnapshot"]>;
  private readonly unitMirror = new Map<string, { readonly status: string }>();
  private primeChildCounter = 0;

  private constructor(options: HarnessOptions) {
    const runId = options.run_id ?? T23_RUN;
    const actor = options.bound_actor ?? T23_ACTOR;
    const token = options.session_token ?? T23_TOKEN;
    this.stub = new CanonicalSurfaceStub({
      run_id: runId,
      bound_actor: actor,
      session_token: token,
    });
    this.seedUnit(T23_RETRY_UNIT, "fail", T23_RETRY_TICKET);
    this.server = this.stub.serveHttp();
    this.httpRoute = new HttpOperatorActionRoute(`127.0.0.1:${this.server.port}`);
    this.shimRoute = this.stub.asInProcessRoute();
    // THE host gateway: one instance per workbench host; projections share it.
    this.gateway = new OperationGateway(this.httpRoute);
    const model = new WorkbenchModel();
    model.startGeneration(GEN);
    model.markReady(GEN);
    this.shellSnapshot = model.getSnapshot();
    this.kernel = new KernelAffordanceDirectory({
      kernel: new PrimeReplKernel("kernel-t23-matrix"),
      gateway: this.gateway,
      route: this.shimRoute,
      context: {
        generation: GEN,
        horizon_id: null,
        kernel_generation: "kernel-gen-t23",
        session_ref: "prime-t23-session",
      },
    });
    this.plugins = new PluginCompositionManager({
      snapshots: model,
      shellViews: createShellViewRegistryCell(defaultViewRegistry()),
    });
    const sample = createSamplePlugin();
    this.plugins.mount(sample.manifest, sample.module);
    this.primeAdapter = new PrimeHostAdapter();
    this.prime = new PrimeDelegationDispatcher({
      adapter: this.primeAdapter,
      canonical_route: this.shimRoute,
      policy: { max_depth: 3, max_children_per_parent: 4 },
    });
  }

  /** A fresh harness over one scenario stub. */
  static build(options: HarnessOptions = {}): CrossSurfaceHarness {
    return new CrossSurfaceHarness(options);
  }

  private seedUnit(unitId: string, status: string, ticket: string | null): void {
    this.stub.addUnit({
      unit_id: unitId,
      status,
      tier: "P3",
      settled: false,
      authority: "granted",
      authority_visible_in_reads: true,
      governance_denied: false,
      governance_visible_in_reads: true,
      open_ticket: ticket,
    });
    this.unitMirror.set(unitId, { status });
  }

  /** Add the DENIED representative's target (blocked unit with an open escalation). */
  addResolveTarget(): this {
    this.seedUnit(T23_DENY_UNIT, "blocked", T23_DENY_TICKET);
    return this;
  }

  /** Release the harness's loopback server. Call exactly once per scenario. */
  close(): void {
    this.server.stop(true);
  }

  // --- scenario knobs (post-render canonical changes) ---------------------------

  /** Revoke a unit's settlement authority; `visibleInReads` models the racing denial. */
  revokeAuthority(unitId: string, visibleInReads: boolean): this {
    this.stub.revokeAuthority(unitId, visibleInReads);
    return this;
  }

  /** Open a governance/residual denial on the unit. */
  denyGovernance(unitId: string, visibleInReads: boolean): this {
    this.stub.denyGovernance(unitId, visibleInReads);
    return this;
  }

  /** Exhaust the run budget as the READS report it (the resolver's payment fact). */
  exhaustBudget(visibleInReads: boolean): this {
    this.stub.exhaustBudget(visibleInReads);
    return this;
  }

  /** Retire the durable run (the anchor the projections held is no longer durable). */
  retireRun(): this {
    this.stub.retireRun();
    return this;
  }

  /**
   * Advance a unit's canonical status AFTER the affordance was rendered
   * (the stale-control). Mirrors the pre-change unit record; scenario use is
   * pre-acceptance only (the mirror does not track settlement).
   */
  setUnitStatus(unitId: string, status: string): this {
    const current = this.unitMirror.get(unitId);
    if (current === undefined) {
      throw new Error(`cross-surface harness: no seeded unit '${unitId}'`);
    }
    this.seedUnit(unitId, status, unitId === T23_RETRY_UNIT ? T23_RETRY_TICKET : T23_DENY_TICKET);
    return this;
  }

  // --- observations -------------------------------------------------------------

  /** A fresh canonical observation through the canonical read path. */
  observe(): AffordanceObservation {
    return this.stub.observe({ unit: null });
  }

  /** Resolve one affordance descriptor from a fresh observation (the render-time prediction). */
  renderDescriptor(
    operation: string,
    focus: FocusRef,
    options: {
      readonly session?: { readonly actor: string; readonly authenticated: boolean } | undefined;
      readonly payment_estimates?:
        | Readonly<Record<string, { tokens: number; cost_usd_micro: number }>>
        | undefined;
    } = {},
  ): AffordanceDescriptor {
    return resolveAffordance(operation, focus, this.observe(), {
      ...(options.session !== undefined ? { session: options.session } : {}),
      ...(options.payment_estimates !== undefined
        ? { payment_estimates: options.payment_estimates }
        : {}),
    });
  }

  /** Frozen copy of the scenario's side-effect ledger. */
  sideEffects(): StubSideEffects {
    return Object.freeze({ ...this.stub.sideEffects });
  }

  /** The Prime adapter's live session count (delegation children + roots). */
  primeSessions(): number {
    return this.primeAdapter.listSessions().length;
  }

  /** The registered affordance-projection views of the mounted sample plugin. */
  pluginProjections(): ReturnType<PluginCompositionManager["affordanceProjections"]> {
    return this.plugins.affordanceProjections();
  }

  // --- the drive ------------------------------------------------------------------

  /** Drive one registered route with one canonical invocation request. */
  async drive(
    route: MatrixRouteId,
    request: OperationInvocationRequest,
  ): Promise<RouteDriveResult> {
    switch (route) {
      case "gateway-http":
        return this.finish(await this.gateway.invoke(request));
      case "gateway-shim":
        return this.finish(await this.gateway.invokeVia(this.shimRoute, request));
      case "kernel-callable":
        return this.driveKernel(request);
      case "plugin-projection":
        return this.drivePlugin(request);
      case "prime-delegation":
        return this.drivePrime(request);
      case "shell-effect":
        return this.shellDrive(request);
      case "direct-api": {
        const [kind, unit, operator, note, run] = this.directArgs(request);
        return this.finish(this.stub.perform(kind, unit, operator, note, run));
      }
      case "direct-cli": {
        const [kind, unit, operator, note] = this.directArgs(request);
        return this.finish(
          this.stub.performViaCli(kind, unit, operator, note, this.cliTicket(request)),
        );
      }
    }
  }

  /**
   * The bare API shape across its transport boundary: the raw HTTP wire with no
   * gateway. Used for the settlement-access variation, where the enforcement
   * lives at the wire's auth gate, not the action body.
   */
  rawApiTransport(command: CanonicalActionCommand): Promise<CanonicalOutcome> {
    return this.httpRoute.perform(command);
  }

  private finish(outcome: GatewayOutcome | CanonicalOutcome): RouteDriveResult {
    return Object.freeze({
      outcome,
      prime_child_spawned: false,
      prime_sessions_total: this.primeSessions(),
      kernel_wrapper_invalidated: false,
    });
  }

  private kernelCallable(
    operation: string,
  ): ReturnType<ReturnType<KernelAffordanceDirectory["current"]>["action"]> {
    return this.kernel.current().action(operation);
  }

  private driveKernel(request: OperationInvocationRequest): Promise<RouteDriveResult> {
    const callable = this.kernelCallable(request.operation);
    if (callable === null) {
      throw new Error(`cross-surface harness: the kernel set exposes no '${request.operation}'`);
    }
    // KernelInvokeRequest carries no resolution field: the drop is structural
    // (pinned by the registry's carries_payment_estimates: false), never hidden.
    const outcome = callable.invoke({
      focus: request.focus,
      session: request.session,
      payload: request.payload,
    });
    return outcome.then((resolved) =>
      Object.freeze({
        outcome: resolved as GatewayOutcome,
        prime_child_spawned: false,
        prime_sessions_total: this.primeSessions(),
        kernel_wrapper_invalidated: callable.invalidation !== null,
      }),
    );
  }

  private drivePlugin(request: OperationInvocationRequest): Promise<RouteDriveResult> {
    const view = this.pluginProjections().find((entry) => entry.operation === request.operation);
    if (view === undefined) {
      throw new Error(`cross-surface harness: no plugin projection for '${request.operation}'`);
    }
    // The host honors the projection's declared estimate at its own gateway
    // drive (the projection predicts; the gateway decides).
    const resolution = view.estimate
      ? { payment_estimates: { [view.operation]: view.estimate } }
      : undefined;
    const outcome = this.gateway.invoke({
      operation: view.operation,
      focus: request.focus,
      session: request.session,
      payload: request.payload,
      ...(request.idempotency_key !== undefined
        ? { idempotency_key: request.idempotency_key }
        : {}),
      ...(resolution !== undefined ? { resolution } : {}),
    });
    return outcome.then((resolved) => this.finish(resolved));
  }

  private drivePrime(request: OperationInvocationRequest): Promise<RouteDriveResult> {
    this.primeChildCounter += 1;
    const delegation = this.prime.delegate({
      operation_request: request,
      envelope: primeEnvelope(),
      child_session_id: `prime-t23-child-${this.primeChildCounter}`,
      parent_session_id: null,
      task: "T23 cross-surface matrix delegation of the frozen representative",
    });
    return delegation.then((outcome) => {
      if (outcome.class === "refused_by_policy") {
        throw new Error(
          `cross-surface harness: the prime route refused by policy (${outcome.code}) — matrix scenarios never trip the policy gates`,
        );
      }
      return Object.freeze({
        outcome: outcome.gateway,
        prime_child_spawned: outcome.class === "admitted",
        prime_sessions_total: this.primeSessions(),
        kernel_wrapper_invalidated: false,
      });
    });
  }

  private shellDrive(request: OperationInvocationRequest): Promise<RouteDriveResult> {
    const observation = this.observe();
    const base = createShellState(
      {
        workspace: { workspace: "gauntlet", reference: "ref-t23-matrix", run: T23_RUN },
      },
      SHELL_DIMS,
    );
    const state: ShellState = Object.freeze({
      ...base,
      location: Object.freeze({ ...base.location, focus: request.focus }),
    });
    const paletteContext = {
      observation,
      ...(request.session !== null
        ? {
            session: {
              actor: request.session.actor,
              authenticated: request.session.token.length > 0,
            },
          }
        : {}),
      ...(request.resolution?.payment_estimates !== undefined
        ? { paymentEstimates: request.resolution.payment_estimates }
        : {}),
    };
    const entries = paletteEntries(state, this.shellSnapshot, paletteContext);
    const index = entries.findIndex(
      (entry) => entry.type === "affordance" && entry.operation === request.operation,
    );
    if (index < 0) {
      throw new Error(
        `cross-surface harness: the palette exposes no '${request.operation}' affordance entry`,
      );
    }
    let next: ShellState = state;
    const open = handleKey(next, this.shellSnapshot, "p", paletteContext);
    next = open.state;
    for (let step = 0; step < index; step += 1) {
      next = handleKey(next, this.shellSnapshot, "down", paletteContext).state;
    }
    const staged = handleKey(next, this.shellSnapshot, "enter", paletteContext);
    // The shell stages consequential invokes: first Enter names the invoke and
    // fires nothing, second Enter performs it. The driver follows the designed
    // interaction and pins both halves of the contract.
    if (staged.effects.some((candidate) => candidate.type === "invoke_affordance")) {
      throw new Error("cross-surface harness: staged Enter must not invoke yet");
    }
    const activated = handleKey(staged.state, this.shellSnapshot, "enter", paletteContext);
    const effect: ShellEffect | undefined = activated.effects.find(
      (candidate) => candidate.type === "invoke_affordance",
    );
    if (effect === undefined || effect.type !== "invoke_affordance") {
      throw new Error("cross-surface harness: palette Enter produced no invoke_affordance effect");
    }
    // The host callback: the ONLY outbound edge. It binds the presented
    // identity (operation + ref from the effect) to the host gateway drive.
    const captured: Promise<GatewayOutcome>[] = [];
    const interactions: ShellInteractions = {
      invokeAffordance: (operation, ref) => {
        captured.push(
          this.gateway.invoke({
            operation,
            focus: ref,
            session: request.session,
            payload: request.payload,
            ...(request.idempotency_key !== undefined
              ? { idempotency_key: request.idempotency_key }
              : {}),
            ...(request.resolution !== undefined ? { resolution: request.resolution } : {}),
          }),
        );
      },
    };
    applyShellEffects(activated.effects, interactions);
    const bound = captured[0];
    if (bound === undefined) {
      throw new Error("cross-surface harness: the shell effect never reached the host callback");
    }
    return bound.then((resolved) => this.finish(resolved));
  }

  private directArgs(
    request: OperationInvocationRequest,
  ): [kind: "resolve" | "retry", unit: string, operator: string, note: string, run: string | null] {
    const spec = lookupOperation(request.operation);
    if (spec === null || !spec.consequential || spec.action_kind === "refresh") {
      throw new Error(
        `cross-surface harness: '${request.operation}' is not a consequential action shape`,
      );
    }
    return [
      spec.action_kind,
      request.focus.source_id,
      request.session?.actor ?? "",
      request.payload.note,
      request.payload.run ?? null,
    ];
  }

  private cliTicket(request: OperationInvocationRequest): string {
    const unit = request.focus.source_id;
    const ticket =
      unit === T23_RETRY_UNIT ? T23_RETRY_TICKET : unit === T23_DENY_UNIT ? T23_DENY_TICKET : null;
    if (ticket === null) {
      throw new Error(`cross-surface harness: no scenario ticket anchors unit '${unit}'`);
    }
    return ticket;
  }
}

/** The bounded context envelope the Prime route delivers to the child (deterministic). */
function primeEnvelope(): PrimeContextEnvelope {
  const world = {
    generation: GEN,
    units: [],
    evidence: [],
    sessions: [],
    source_cursors: {},
  };
  const horizon = createHorizon({
    root_focus: FocusRef.for("gauntlet", "WorkItem", T23_RETRY_UNIT),
    generation: GEN,
    relation_queries: [],
    horizon_id: "horizon-t23-matrix",
  });
  return constructBoundedContextEnvelope({
    horizon,
    plan: planMaterialization(world, horizon),
    role_instructions: ["T23 cross-surface matrix probe"],
    constructed_at_ms: 1_800_000_000_000,
  });
}

// --- Request factories ------------------------------------------------------------

/** The frozen ALLOWED representative request (retry of the failed unit, anchored on the durable run). */
export function allowedRetryRequest(
  overrides: Partial<OperationInvocationRequest> = {},
): OperationInvocationRequest {
  return {
    operation: OPERATION_IDS.UNIT_RETRY,
    focus: FocusRef.for("gauntlet", "WorkItem", T23_RETRY_UNIT),
    session: { actor: T23_ACTOR, token: T23_TOKEN },
    payload: { note: "T23 allowed representative: retry the failed unit", run: T23_RUN },
    ...overrides,
  };
}

/** The frozen DENIED representative request (resolve of the escalation). */
export function deniedResolveRequest(
  overrides: Partial<OperationInvocationRequest> = {},
): OperationInvocationRequest {
  return {
    operation: OPERATION_IDS.UNIT_RESOLVE,
    focus: FocusRef.for("gauntlet", "WorkItem", T23_DENY_UNIT),
    session: { actor: T23_ACTOR, token: T23_TOKEN },
    payload: { note: "T23 denied representative: resolve the escalation", run: T23_RUN },
    ...overrides,
  };
}
