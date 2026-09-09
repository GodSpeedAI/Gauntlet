/**
 * Kernel callable affordances (plan T11, REQ-AFF-002, REQ-OUT-004,
 * REQ-PRIME-006, REQ-SYNC-003 direction).
 *
 * Projects the currently admitted deterministic operations of the canonical
 * registry (T09) into the Prime kernel as callable affordances. The wrappers
 * are PROJECTIONS, never authority:
 *
 *   - Every callable carries a canonical operation ID from `operation-registry`
 *     and routes through the ONE `OperationGateway` (T09) on EVERY invocation.
 *     No business logic lives in a wrapper; a kernel call converges on the same
 *     admission, validation, authority, idempotency, and result semantics as
 *     TUI controls, palette entries, and CLI/API projections (REQ-AFF-002,
 *     REQ-OUT-004). There is no privileged kernel side path: a stale wrapper's
 *     invocation re-enters canonical admission, is denied by the fresh gate,
 *     and only then is the wrapper marked invalidated — wrapper existence never
 *     bypasses gates (REQ-AFF-003).
 *   - Read/query helpers are represented SEPARATELY from consequential
 *     affordances: a distinct type with a distinct surface, served by the
 *     canonical READ path (the same route transport the gateway routes over),
 *     never the action admission path, and typed as affecting nothing.
 *   - Wrappers invalidate and refresh on generation/Horizon/authority change.
 *     Invalidation is a PROJECTION update: it retires the stale namespace and
 *     regenerates a current one. It never removes routing through the gateway —
 *     only the gateway's invocation-time revalidation and canonical admission
 *     decide (defense in depth; the invalidation cannot grant anything).
 *   - Kernel input/control needs (REQ-PRIME-006) surface through an explicit
 *     workbench Attention sink and return through the Prime-owned Python REPL
 *     mediation path (T05's serialized `kernel.execute`). There is NO direct
 *     prompting: with no attention sink registered the request is typed
 *     unroutable — never satisfied from the terminal.
 *
 * All identifiers are minted from deterministic per-process counters (no
 * wall clock), so tests are deterministic in-memory.
 */

import type { FocusRef } from "./focus";
import type {
  GatewayOutcome,
  OperationGateway,
  OperationPayload,
  OperationSession,
  OperatorActionRoute,
} from "./operation-gateway";
import type { OperationId, OperationSpec } from "./operation-registry";
import { OPERATION_REGISTRY, operationIds } from "./operation-registry";
import type { PrimeReplKernel, ReplExecuteResult } from "./prime-kernel";

// --- Context and wrapper state -------------------------------------------------

/** The current-world context a kernel affordance set is generated FOR. */
export interface KernelAffordanceContext {
  /** Active workbench projection generation the set was generated under. */
  readonly generation: string;
  /** Active Horizon id, or null for the whole-Case (unhoisted) world. */
  readonly horizon_id: string | null;
  /** Owning Prime runtime incarnation (KernelObjectRef.kernel_generation semantics). */
  readonly kernel_generation: string;
  /** Owning Prime session reference. */
  readonly session_ref: string;
}

/** Lifecycle state of one generated wrapper. */
export type KernelCallableState = "current" | "stale" | "invalidated";

/** One recorded invalidation: why the wrapper was retired. */
export interface KernelWrapperInvalidation {
  readonly reason: string;
  /** The canonical refusal code when an invocation produced the invalidation. */
  readonly canonical_code: string | null;
}

/** The payload an kernel-side caller supplies for one action invocation. */
export type KernelInvokeRequest = {
  readonly focus: FocusRef;
  readonly session: OperationSession | null;
  readonly payload: OperationPayload;
};

// --- Consequential callables ---------------------------------------------------

/**
 * One consequential kernel callable bound to a canonical operation. Invoking
 * routes through the T09 gateway — the SAME canonical gateway every other
 * projection uses — and the canonical outcome is returned verbatim. A refused
 * STALE outcome additionally invalidates this wrapper (observable projection
 * state), never the authority decision itself.
 */
export class KernelActionCallable {
  readonly operation: OperationId;
  readonly spec: OperationSpec;
  /** Literal kind tag: consequential actions and read helpers are disjoint types. */
  readonly kind = "consequential" as const;
  private set: KernelAffordanceSet;

  constructor(operation: OperationId, spec: OperationSpec, set: KernelAffordanceSet) {
    this.operation = operation;
    this.spec = spec;
    this.set = set;
  }

  /** Whether this wrapper is still current (projection state only — never a gate). */
  get state(): KernelCallableState {
    return this.set.state === "current" ? "current" : this.set.state;
  }

  get invalidation(): KernelWrapperInvalidation | null {
    return this.set.invalidation;
  }

  /**
   * Invoke the canonical operation through the T09 gateway. The gateway's
   * invocation-time revalidation decides — this wrapper's existence, currency,
   * or staleness contributes NOTHING to the decision (REQ-AFF-003, REQ-OUT-004:
   * no privileged side path).
   */
  async invoke(request: KernelInvokeRequest): Promise<GatewayOutcome> {
    const outcome = await this.set.invoke(this.operation, request);
    // A pre-admission stale denial retires the STALE WRAPPER (the projection),
    // so the kernel environment sees its callable go away — the canonical gates
    // already decided; this records the projection consequence only.
    if (outcome.class === "refused" && outcome.stale) {
      this.set.recordInvalidation({
        reason: `invocation denied pre-admission (${outcome.code})`,
        canonical_code: outcome.code,
      });
    }
    return outcome;
  }
}

// --- Read/query helpers (separate from consequential affordances) ---------------

/** One canonical read the observation carries, verbatim. */
export type KernelReadOutcome =
  | {
      readonly class: "observation";
      readonly operation: OperationId;
      /** Literal false at the type level: a read helper affects no canonical state. */
      readonly affects_canonical_state: false;
      readonly observation: Awaited<ReturnType<OperatorActionRoute["observe"]>>;
    }
  | {
      readonly class: "refused";
      readonly operation: OperationId;
      readonly affects_canonical_state: false;
      readonly code: "read_unavailable";
      readonly message: string;
    };

/**
 * One read/query kernel helper bound to a read-shaped registry operation.
 * Deliberately a DIFFERENT type from `KernelActionCallable` with NO invoke
 * method: a read helper cannot be admitted through the action path (the
 * registry and gateway type such a route `not_action_admitted`), and it never
 * mutates anything — it is served by the canonical read path over the SAME
 * route transport the gateway routes consequential calls through.
 */
export class KernelReadHelper {
  readonly operation: OperationId;
  readonly spec: OperationSpec;
  /** Literal kind tag: disjoint from `consequential` at the type level. */
  readonly kind = "read" as const;
  /** Literal false: query helpers affect nothing. */
  readonly affects_canonical_state = false as const;
  private readonly route: OperatorActionRoute;

  constructor(operation: OperationId, spec: OperationSpec, route: OperatorActionRoute) {
    this.operation = operation;
    this.spec = spec;
    this.route = route;
  }

  /**
   * Read through the canonical read path. A read that cannot be observed fails
   * CLOSED with the typed `read_unavailable` refusal — never a fabricated
   * observation, never a retry against the action admission path.
   */
  async read(target: { unit: string | null }): Promise<KernelReadOutcome> {
    try {
      const observation = await this.route.observe(target);
      return Object.freeze({
        class: "observation" as const,
        operation: this.operation,
        affects_canonical_state: false as const,
        observation,
      });
    } catch (error) {
      return Object.freeze({
        class: "refused" as const,
        operation: this.operation,
        affects_canonical_state: false as const,
        code: "read_unavailable" as const,
        message: `the canonical read path could not serve '${this.operation}': ${
          error instanceof Error ? error.message : String(error)
        }`,
      });
    }
  }
}

// --- Generated affordance sets --------------------------------------------------

/**
 * One generated kernel affordance namespace: the current callables for one
 * (generation, horizon, kernel runtime) context. Generated FROM the registry —
 * never hand-registered — so the kernel can never know an operation the
 * canonical surface does not serve.
 */
export class KernelAffordanceSet {
  readonly context: KernelAffordanceContext;
  /** Consequential callables (registry operations admitted through the action path). */
  readonly actions: readonly KernelActionCallable[];
  /** Read/query helpers (registry operations served by the canonical read path). */
  readonly reads: readonly KernelReadHelper[];
  private setState: KernelCallableState = "current";
  private setInvalidation: KernelWrapperInvalidation | null = null;
  private readonly gateway: OperationGateway;
  private readonly staleReasons: string[] = [];

  constructor(
    context: KernelAffordanceContext,
    gateway: OperationGateway,
    route: OperatorActionRoute,
  ) {
    this.context = context;
    this.gateway = gateway;
    const actions: KernelActionCallable[] = [];
    const reads: KernelReadHelper[] = [];
    for (const id of operationIds()) {
      const spec = OPERATION_REGISTRY[id];
      if (spec.consequential) {
        actions.push(new KernelActionCallable(id, spec, this));
      } else {
        reads.push(new KernelReadHelper(id, spec, route));
      }
    }
    this.actions = Object.freeze(actions);
    this.reads = Object.freeze(reads);
  }

  /** Projection currency of the whole set (never a gate — see `invoke`). */
  get state(): KernelCallableState {
    return this.setState;
  }

  get invalidation(): KernelWrapperInvalidation | null {
    return this.setInvalidation;
  }

  get stale_reasons(): readonly string[] {
    return this.staleReasons;
  }

  isCurrent(): boolean {
    return this.state === "current";
  }

  /** Find the consequential callable for one canonical operation. */
  action(operation: string): KernelActionCallable | null {
    return this.actions.find((callable) => callable.operation === operation) ?? null;
  }

  /** Find the read helper for one read-shaped operation. */
  read(operation: string): KernelReadHelper | null {
    return this.reads.find((helper) => helper.operation === operation) ?? null;
  }

  /**
   * Mark the set stale (context changed under it). Projection bookkeeping only:
   * callables of a stale set STILL route through the gateway and re-enter
   * canonical admission; nothing is bypassed by being stale.
   */
  markStale(reason: string): void {
    if (this.setState === "current") {
      this.setState = "stale";
    }
    this.staleReasons.push(reason);
  }

  /** Record an invalidation (e.g. a stale-denied invocation retired the wrapper). */
  recordInvalidation(invalidation: KernelWrapperInvalidation): void {
    this.setState = "invalidated";
    this.setInvalidation = Object.freeze({ ...invalidation });
  }

  /**
   * The ONE routing path for kernel action calls: the T09 gateway, verbatim.
   * Every callable of this set — current, stale, or invalidated — invokes
   * through here; the gateway's fresh revalidation decides every time.
   */
  invoke(operation: OperationId, request: KernelInvokeRequest): Promise<GatewayOutcome> {
    return this.gateway.invoke({
      operation,
      focus: request.focus,
      session: request.session,
      payload: request.payload,
    });
  }
}

// --- The directory: generation, invalidation, refresh ---------------------------

/**
 * Generates the CURRENT kernel affordance set for the active context and
 * invalidates/refreshes on generation, Horizon, or authority change. The
 * directory owns NO authority: it is bookkeeping over projections, and the
 * gateway revalidates every invocation regardless of what the directory
 * believes is current.
 */
export class KernelAffordanceDirectory {
  private readonly kernel: PrimeReplKernel;
  private readonly gateway: OperationGateway;
  private readonly route: OperatorActionRoute;
  private live: KernelAffordanceSet;
  private context: KernelAffordanceContext;
  private setCounter = 0;

  constructor(deps: {
    kernel: PrimeReplKernel;
    gateway: OperationGateway;
    route: OperatorActionRoute;
    context: KernelAffordanceContext;
  }) {
    this.kernel = deps.kernel;
    this.gateway = deps.gateway;
    this.route = deps.route;
    this.context = deps.context;
    this.live = this.generate();
  }

  /** The current set; regenerated automatically when the context moved. */
  current(): KernelAffordanceSet {
    return this.live;
  }

  /**
   * Workbench generation changed: the live set is stale and a fresh set is
   * generated for the new generation. Returns the NEW current set.
   */
  notifyGenerationChanged(generation: string): KernelAffordanceSet {
    if (generation === this.context.generation) {
      return this.live;
    }
    this.live.markStale(`workbench generation changed ${this.context.generation} -> ${generation}`);
    this.context = { ...this.context, generation };
    this.live = this.generate();
    return this.live;
  }

  /** Horizon changed (hoist/de-hoist): same invalidation/refresh discipline. */
  notifyHorizonChanged(horizonId: string | null): KernelAffordanceSet {
    if (horizonId === this.context.horizon_id) {
      return this.live;
    }
    this.live.markStale(
      `horizon changed ${String(this.context.horizon_id)} -> ${String(horizonId)}`,
    );
    this.context = { ...this.context, horizon_id: horizonId };
    this.live = this.generate();
    return this.live;
  }

  /**
   * Authority changed (revocation/grant observed by the host): the projection
   * refreshes. This is NOT the enforcement point — the gateway's fresh
   * revalidation enforces authority at every invocation whether or not this
   * notification ever fires.
   */
  notifyAuthorityChanged(): KernelAffordanceSet {
    this.live.markStale("authority changed under the active affordance set");
    this.live = this.generate();
    return this.live;
  }

  private generate(): KernelAffordanceSet {
    this.setCounter += 1;
    return new KernelAffordanceSet(this.context, this.gateway, this.route);
  }

  /** Diagnostic: which Prime kernel and generation the directory projects for. */
  describe(): { kernel_session: string; kernel_generation: string; sets_generated: number } {
    return {
      kernel_session: this.kernel.sessionId,
      kernel_generation: this.context.kernel_generation,
      sets_generated: this.setCounter,
    };
  }
}

// --- Kernel input through Attention/Prime mediation (REQ-PRIME-006) -------------

/**
 * One kernel input need, routed to the workbench Attention surface. NEVER a
 * terminal prompt: there is no prompting API in this module at all.
 */
export interface KernelInputRequest {
  readonly request_id: string;
  readonly name: string;
  readonly prompt: string;
  readonly kernel_session: string;
  readonly kernel_generation: string;
}

/** What the Attention surface received (mediation provenance). */
export type KernelInputDelivery =
  | {
      readonly class: "queued_for_attention";
      readonly request: KernelInputRequest;
      /** The explicit surface the request waits on. */
      readonly mediation_path: "workbench_attention";
      readonly returns_through: "prime_python_repl_mediation";
    }
  | {
      readonly class: "unroutable";
      readonly request: KernelInputRequest;
      /** Fail-closed: without an Attention sink the request stays pending — never a direct prompt. */
      readonly reason: "no_attention_sink_registered";
      readonly mediation_path: "none";
    };

/** The answer to one kernel input request. */
export type KernelInputResolution =
  | {
      readonly class: "delivered_via_kernel_mediation";
      readonly request: KernelInputRequest;
      /** The serialized T05 execution that carried the answer INTO the kernel. */
      readonly execution: ReplExecuteResult;
    }
  | {
      readonly class: "refused";
      readonly code: "unknown_request" | "already_responded";
      readonly message: string;
    };

/** The workbench Attention sink: registered by the workbench's attention surface. */
export type KernelAttentionSink = (request: KernelInputRequest) => void;

/**
 * Routes kernel input/control needs through the explicit Attention path and
 * returns answers through the Prime-owned Python REPL mediation path (T05's
 * serialized `kernel.execute`). The broker cannot prompt: with no sink, the
 * request is typed `unroutable` and stays pending until an operator reaches it
 * through the workbench surface.
 */
export class KernelInputBroker {
  private readonly kernel: PrimeReplKernel;
  private readonly sink: KernelAttentionSink | null;
  private readonly kernelGeneration: string;
  private readonly pendingRequests = new Map<string, KernelInputRequest>();
  private readonly responded = new Set<string>();
  private counter = 0;

  constructor(deps: {
    kernel: PrimeReplKernel;
    /** The registered Attention surface, or null while none is attached. */
    attention_sink: KernelAttentionSink | null;
    kernel_generation: string;
  }) {
    this.kernel = deps.kernel;
    this.sink = deps.attention_sink;
    this.kernelGeneration = deps.kernel_generation;
  }

  /**
   * Surface one kernel input need. Delivered to the Attention sink when one is
   * registered; typed `unroutable` (and left pending) otherwise. Never prompts.
   */
  request(name: string, prompt: string): KernelInputDelivery {
    if (name.trim().length === 0) {
      throw new Error("kernel input requests require a non-empty target variable name");
    }
    this.counter += 1;
    const req: KernelInputRequest = Object.freeze({
      request_id: `kir-${this.kernel.sessionId}-${this.counter}`,
      name,
      prompt,
      kernel_session: this.kernel.sessionId,
      kernel_generation: this.kernelGeneration,
    });
    if (this.sink === null) {
      this.pendingRequests.set(req.request_id, req);
      return Object.freeze({
        class: "unroutable" as const,
        request: req,
        reason: "no_attention_sink_registered" as const,
        mediation_path: "none" as const,
      });
    }
    this.pendingRequests.set(req.request_id, req);
    this.sink(req);
    return Object.freeze({
      class: "queued_for_attention" as const,
      request: req,
      mediation_path: "workbench_attention" as const,
      returns_through: "prime_python_repl_mediation" as const,
    });
  }

  /** Requests awaiting an operator answer through the Attention surface. */
  pending(): readonly KernelInputRequest[] {
    return Object.freeze([...this.pendingRequests.values()]);
  }

  /**
   * Deliver the operator's answer INTO the kernel through the Prime-owned
   * mediation path (the serialized REPL execution) — the answer never comes
   * from and never goes to anywhere but Attention → Prime mediation.
   */
  async respond(requestId: string, value: string): Promise<KernelInputResolution> {
    if (this.responded.has(requestId)) {
      return Object.freeze({
        class: "refused" as const,
        code: "already_responded" as const,
        message: `kernel input request '${requestId}' was already answered`,
      });
    }
    const req = this.pendingRequests.get(requestId);
    if (req === undefined) {
      return Object.freeze({
        class: "refused" as const,
        code: "unknown_request" as const,
        message: `kernel input request '${requestId}' was never issued by this broker`,
      });
    }
    this.responded.add(requestId);
    this.pendingRequests.delete(requestId);
    const execution = await this.kernel.execute(`${req.name} = ${value}`);
    return Object.freeze({
      class: "delivered_via_kernel_mediation" as const,
      request: req,
      execution,
    });
  }
}
