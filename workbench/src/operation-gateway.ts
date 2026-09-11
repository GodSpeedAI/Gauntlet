/**
 * The ONE isomorphic operation gateway (plan T09, REQ-AFF-001/002/003,
 * REQ-OPS-002/003, REQ-CONFIG-011, REQ-OUT-004, REQ-SAFE-002, REQ-SEC-002,
 * REQ-VERIFY-003/004).
 *
 * Every workbench-side invocation of a consequential operation routes through
 * this gateway to canonical admission — the Rust surface's ONE operator-action
 * path (crates/gauntlet-cli/src/durable_surface.rs `perform` →
 * operator_action::execute_operator_action → the settlement service). The
 * gateway is a ROUTER, never a second authority:
 *
 *   - It never decides allow/deny from workbench-local state. Its pre-admission
 *     revalidation resolves a FRESH AffordanceDescriptor from a FRESH canonical
 *     observation obtained through the SAME route the command would take; a
 *     surface that cannot be observed fails CLOSED for EVERY drive — fresh
 *     invocation, §16.3 re-drive, and reconciliation alike — and a fresh
 *     unavailable/degraded resolution fails CLOSED before any command is sent
 *     (REQ-AFF-003, REQ-CONFIG-011, REQ-SAFE-002). Canonical admission
 *     re-checks independently regardless (defense in depth).
 *   - Each invocation binds actor, session, target, payload digest, and an
 *     idempotency key (REQ-OPS-002). The key is DETERMINISTIC and never a
 *     secret — the defense against misuse is the gate, not key obscurity.
 *     Key reuse with a changed payload is refused before routing. A
 *     lost/ambiguous transport response resolves to the typed
 *     `outcome_unknown` and is never silently retried under a new key;
 *     reconciliation re-drives the SAME key through canonical re-entry. Only a
 *     key's own durable canonical record (an outcome still unknown, or an
 *     accepted settlement) authorizes skipping the fresh descriptor gate for
 *     the §16.3 re-entry; refused and failed outcomes arm nothing, and the
 *     bind→observe→gate decision is serialized per host (Correction Round 1).
 *   - The canonical result (accepted / refused / failed) is reported with its
 *     canonical operation identity and refusal code preserved — every route
 *     converges on one admission, authority, idempotency, and result shape
 *     (REQ-AFF-002, REQ-VERIFY-003/004).
 */

import {
  type AffordanceDescriptor,
  type AffordanceObservation,
  type ResolveAffordanceOptions,
  resolveAffordance,
  statusPermitsInvocation,
} from "./affordance";
import type { FocusRef } from "./focus";
import type { OperationSpec } from "./operation-registry";
import { lookupOperation, type OperationId } from "./operation-registry";

/** The actor/session binding an invocation carries (T01's config/auth vocabulary). */
export interface OperationSession {
  readonly actor: string;
  readonly token: string;
}

/** The operator-provided payload of one invocation. */
export interface OperationPayload {
  readonly note: string;
  readonly run?: string | undefined;
}

/** One invocation request as a projection submits it (registry ID + focus + binding). */
export interface OperationInvocationRequest {
  readonly operation: string;
  readonly focus: FocusRef;
  readonly session: OperationSession | null;
  readonly payload: OperationPayload;
  /** Present when reconciling or re-driving a KNOWN key. Never mint a second key for the same logical invocation. */
  readonly idempotency_key?: string | undefined;
  /** Resolution options carried into revalidation (payment estimates, presentation hint). */
  readonly resolution?: Omit<ResolveAffordanceOptions, "session"> | undefined;
}

/** The canonical command shape, mirroring ActionCommand plus the workbench binding. */
export interface CanonicalActionCommand {
  readonly operation: OperationId;
  readonly action_kind: "resolve" | "retry";
  readonly unit: string;
  readonly operator: string;
  readonly session_token: string;
  readonly note: string;
  readonly run: string | null;
  readonly payload_digest: string;
  readonly idempotency_key: string;
}

/** The canonical outcome vocabulary (ActionOutcome: Accepted | Refused | Failed). */
export type CanonicalOutcome =
  | {
      readonly kind: "accepted";
      readonly run_id: string;
      readonly unit_id: string;
      readonly tickets_resolved: readonly string[];
      readonly committed_now: boolean;
    }
  | {
      readonly kind: "refused";
      readonly code: string;
      readonly message: string;
    }
  | {
      readonly kind: "failed";
      readonly code: string;
      readonly message: string;
    };

/**
 * One route to canonical admission. Implemented by the HTTP surface transport
 * (loopback `gauntlet serve`), by in-process adapter shims, and by test
 * stand-ins — all terminating in the ONE canonical admission path.
 */
export interface OperatorActionRoute {
  /**
   * A FRESH canonical observation for the invocation's target unit (null for
   * Case-scoped). Throws when the surface cannot be observed — the gateway
   * fails closed on that, never invokes.
   */
  observe(target: { unit: string | null }): Promise<AffordanceObservation>;
  /** Perform one canonical action. Throws on lost/ambiguous transport (→ outcome_unknown). */
  perform(command: CanonicalActionCommand): Promise<CanonicalOutcome>;
}

/** The gateway's outcome, carrying the canonical operation identity on every class. */
export type GatewayOutcome =
  | {
      readonly class: "accepted";
      readonly operation: OperationId;
      readonly idempotency_key: string;
      readonly run_id: string;
      readonly unit_id: string;
      readonly tickets_resolved: readonly string[];
      readonly committed_now: boolean;
    }
  | {
      readonly class: "refused";
      readonly operation: OperationId | string;
      readonly idempotency_key: string;
      /** The canonical refusal class token (or the gateway's typed pre-admission class). */
      readonly code: string;
      readonly message: string;
      /** Preserved observation reason codes for pre-admission (stale) denials. */
      readonly reason_codes: readonly string[];
      /** True when the denial happened BEFORE canonical admission (fresh revalidation fail-closed). */
      readonly stale: boolean;
      readonly descriptor: AffordanceDescriptor | null;
    }
  | {
      readonly class: "failed";
      readonly operation: OperationId;
      readonly idempotency_key: string;
      readonly code: string;
      readonly message: string;
    }
  | {
      readonly class: "outcome_unknown";
      readonly operation: OperationId;
      readonly idempotency_key: string;
      readonly detail: string;
    };

/** The idempotency-key identity of one logical invocation (REQ-OPS-002). */
function payloadDigest(payload: OperationPayload): string {
  // FNV-1a 32-bit, doubled with distinct bases to form a 16-hex-digit digest.
  // A binding/mismatch digest, not a cryptographic instrument.
  const text = JSON.stringify({ note: payload.note, run: payload.run ?? null });
  let h1 = 0x811c9dc5;
  let h2 = 0x01000193;
  for (let index = 0; index < text.length; index += 1) {
    const code = text.charCodeAt(index);
    h1 ^= code;
    h1 = Math.imul(h1, 0x01000193) >>> 0;
    h2 ^= (code + index) & 0xff;
    h2 = Math.imul(h2, 0x85ebca6b) >>> 0;
  }
  return h1.toString(16).padStart(8, "0") + h2.toString(16).padStart(8, "0");
}

function mintIdempotencyKey(
  operation: OperationId,
  target: string,
  actor: string,
  digest: string,
): string {
  return `${operation}:${target}:${actor}:${digest}`;
}

const PRE_ADMISSION_CLASS = "stale_affordance_denied";

/**
 * The canonical operation gateway. One instance per workbench host; projections
 * share it. Constructed over ONE route to canonical admission.
 */
export class OperationGateway {
  private readonly route: OperatorActionRoute;
  /**
   * Binding memory: every PRESENTED key → its payload digest, armed at bind
   * time (REQ-OPS-002: key reuse with a different payload is rejected). This
   * ledger is a MISMATCH-DETECTION memory ONLY — arming it confers NO
   * re-drive privilege (Correction Round 1 BLOCKING 1: a refused invocation
   * must never arm a gate skip).
   */
  private readonly keyLedger = new Map<string, string>();
  /** Keys whose outcome is unknown (lost/ambiguous response), awaiting reconciliation. */
  private readonly unknownOutcomes = new Set<string>();
  /**
   * Keys whose command obtained a DURABLE canonical resolution: an ACCEPTED
   * settlement only (the §16.3 re-entry record). Together with
   * `unknownOutcomes` these are the ONLY re-drive authorizations; refused AND
   * failed outcomes arm nothing (Correction Round 2: a failed drive committed
   * nothing, so an armed retry would be a fresh admission that CAN commit —
   * the accepted-arm idempotence argument does not transfer to failed).
   */
  private readonly admittedKeys = new Set<string>();
  /**
   * In-host decision serialization (Correction Round 1 BLOCKING 3): the
   * bind → observe → gate → ledger decision of one invocation is atomic with
   * respect to every other drive on this gateway, so two concurrent invocations
   * of one request can never interleave an arm with a gate skip.
   */
  private decisionTail: Promise<unknown> = Promise.resolve();

  constructor(route: OperatorActionRoute) {
    this.route = route;
  }

  /** Whether a key is currently awaiting outcome reconciliation. */
  isOutcomeUnknown(idempotencyKey: string): boolean {
    return this.unknownOutcomes.has(idempotencyKey);
  }

  /**
   * Serialize one drive's decision (Correction Round 1 BLOCKING 3). Each
   * queued job runs only after the previous decision fully resolved, and a
   * rejected job never poisons the tail.
   */
  private enqueueDecision<T>(job: () => Promise<T>): Promise<T> {
    const run = this.decisionTail.then(
      () => job(),
      () => job(),
    );
    this.decisionTail = run.catch(() => undefined);
    return run;
  }

  /**
   * Invoke one operation through the gateway's configured route.
   * Sequence: registry lookup → binding → FRESH observation (fail-closed on
   * EVERY drive) → fresh descriptor gate (unless the key's own durable
   * canonical record authorizes §16.3 re-entry) → route to canonical
   * admission → typed canonical outcome.
   */
  invoke(request: OperationInvocationRequest): Promise<GatewayOutcome> {
    return this.enqueueDecision(() => this.dispatch(request, this.route));
  }

  /**
   * Invoke through an ALTERNATE canonical admission route of the same host
   * (e.g. the in-process adapter shim beside the HTTP surface). The key
   * ledger, revalidation, and outcome semantics are the gateway's — routes are
   * interchangeable transports to the ONE canonical admission, never separate
   * authorities.
   */
  invokeVia(
    route: OperatorActionRoute,
    request: OperationInvocationRequest,
  ): Promise<GatewayOutcome> {
    return this.enqueueDecision(() => this.dispatch(request, route));
  }

  /**
   * One serialized drive (Correction Round 1 BLOCKING 1/2/3). The decision —
   * bind, observe, gate, ledger, route — is atomic with respect to every
   * other drive on this gateway.
   */
  private async dispatch(
    request: OperationInvocationRequest,
    route: OperatorActionRoute,
  ): Promise<GatewayOutcome> {
    // 1. Registry gate: unsupported operations render unsupported, never a
    //    fallback (REQ-OPS-003). A REGISTERED but read-shaped operation
    //    (state.refresh) is not admitted through the action path at all —
    //    the distinct typed `not_action_admitted` refusal, never
    //    unsupported and never routed (Correction Round 1 MINOR 2).
    const spec: OperationSpec | null = lookupOperation(request.operation);
    if (spec === null) {
      return this.refusedBeforeAdmission(request, "", {
        code: "unsupported_operation",
        message: `operation '${request.operation}' is not a canonical operation of this surface`,
        reasonCodes: ["unsupported_operation"],
        descriptor: null,
      });
    }
    if (!spec.consequential) {
      return this.refusedBeforeAdmission(request, "", {
        code: "not_action_admitted",
        message: `operation '${request.operation}' is registered but read-shaped; it is served by the canonical read path, not the action admission path`,
        reasonCodes: ["not_action_admitted"],
        descriptor: null,
      });
    }

    // 2. Binding: actor/session/target/payload digest/idempotency key. The
    //    ledger arms at bind time as MISMATCH-DETECTION memory only — it
    //    never confers re-drive privilege (Correction Round 1 BLOCKING 1).
    const digest = payloadDigest(request.payload);
    const target = request.focus.source_id;
    const key =
      request.idempotency_key ??
      mintIdempotencyKey(spec.id, target, request.session?.actor ?? "", digest);

    const knownDigest = this.keyLedger.get(key);
    if (knownDigest !== undefined && knownDigest !== digest) {
      // Key reuse with a different payload MUST be rejected (REQ-OPS-002) —
      // before any routing, with zero side effects.
      return this.refusedBeforeAdmission(request, key, {
        code: "idempotency_key_payload_mismatch",
        message: `idempotency key '${key}' was issued for a different payload`,
        reasonCodes: ["idempotency_key_payload_mismatch"],
        descriptor: null,
      });
    }

    // 3. Invocation-time revalidation (REQ-AFF-003, REQ-CONFIG-011) — on
    //    EVERY drive, without exception (Correction Round 1 BLOCKING 2): a
    //    surface that cannot be observed fails closed with the typed
    //    `revalidation_unavailable` refusal, for fresh invocations, §16.3
    //    re-drives, and reconciliation alike. A read that answers but reports
    //    itself unreachable is treated exactly like a failed read. The
    //    gateway never grants authority — it only refuses to route when the
    //    canonical surface's own current answer cannot be obtained or is no.
    let observation: AffordanceObservation;
    try {
      observation = await route.observe({ unit: spec.unit_scoped ? target : null });
    } catch (error) {
      return this.refusedBeforeAdmission(request, key, {
        code: "revalidation_unavailable",
        message: `the canonical surface could not be revalidated before invocation: ${describeError(error)}`,
        reasonCodes: ["surface_unreachable"],
        descriptor: null,
      });
    }
    if (!observation.reachable) {
      return this.refusedBeforeAdmission(request, key, {
        code: "revalidation_unavailable",
        message:
          "the canonical surface could not be revalidated before invocation: the observation reports an unreachable surface",
        reasonCodes: ["surface_unreachable"],
        descriptor: null,
      });
    }

    // 4. The fresh descriptor gate — unless the key's OWN durable canonical
    //    record authorizes §16.3 re-entry (Correction Round 1 BLOCKING 1).
    //    Only two records authorize the skip: an outcome still unknown
    //    (reconciliation of a lost response) and an ACCEPTED settlement
    //    (re-entry reports the durable truth — a settled unit no longer
    //    shows an open ticket, and that projection change must never block
    //    reporting committed_now=false). A REFUSED or FAILED invocation arms
    //    nothing: re-invoking it is a fresh gate pass, whoever supplies the
    //    key and however deterministically the minted key could be recomputed
    //    (the key is NOT a secret — the gate, never key secrecy, is the
    //    defense).
    const reDriveAuthorized = this.unknownOutcomes.has(key) || this.admittedKeys.has(key);
    if (!reDriveAuthorized) {
      const descriptor = resolveAffordance(request.operation, request.focus, observation, {
        session:
          request.session === null
            ? undefined
            : {
                actor: request.session.actor,
                // A presented session is authenticated only when it carries
                // credentials; the canonical surface re-validates the token
                // regardless (resolution predicts, admission decides).
                authenticated: request.session.token.length > 0,
              },
        payment_estimates: request.resolution?.payment_estimates,
        presentation_hint: request.resolution?.presentation_hint,
      });
      if (!statusPermitsInvocation(descriptor.status)) {
        return this.refusedBeforeAdmission(request, key, {
          code: PRE_ADMISSION_CLASS,
          message: `the affordance resolved ${descriptor.status} at invocation time (was rendered available): ${descriptor.reason_codes.join(", ")}`,
          reasonCodes: descriptor.reason_codes,
          descriptor,
        });
      }
    }

    // 5. Route to canonical admission; report the canonical result.
    this.keyLedger.set(key, digest);
    const command: CanonicalActionCommand = {
      operation: spec.id,
      action_kind: spec.action_kind as "resolve" | "retry",
      unit: target,
      operator: request.session?.actor ?? "",
      session_token: request.session?.token ?? "",
      note: request.payload.note,
      run: request.payload.run ?? null,
      payload_digest: digest,
      idempotency_key: key,
    };
    try {
      const outcome = await route.perform(command);
      this.unknownOutcomes.delete(key);
      // Only an ACCEPTED settlement arms the §16.3 re-entry record (Correction
      // Round 2): a FAILED drive committed nothing, so arming it would let the
      // retry skip the fresh gate and execute a fresh, committable admission.
      // Refused and failed outcomes arm nothing.
      if (outcome.kind === "accepted") {
        this.admittedKeys.add(key);
      }
      return this.admit(spec.id, key, outcome);
    } catch (error) {
      // Lost/ambiguous response: typed outcome_unknown under the SAME key —
      // never silently retried with a new key (REQ-OPS-002).
      this.unknownOutcomes.add(key);
      return Object.freeze({
        class: "outcome_unknown" as const,
        operation: spec.id,
        idempotency_key: key,
        detail: `the admission response was lost or ambiguous: ${describeError(error)}`,
      });
    }
  }

  /**
   * Reconcile a key whose outcome is unknown: re-drive the SAME key with the
   * SAME payload through canonical re-entry. A changed payload or an unknown
   * key is refused; no new key is ever minted here. The descriptor gate is
   * deliberately not re-applied (see dispatch): canonical idempotent re-entry
   * and its authority gates decide the durable truth. Observability is still
   * enforced on every drive (Correction Round 1 BLOCKING 2): a surface that
   * cannot be observed fails closed even for reconciliation.
   */
  reconcile(idempotencyKey: string, request: OperationInvocationRequest): Promise<GatewayOutcome> {
    return this.enqueueDecision(async () => {
      const withKey: OperationInvocationRequest = { ...request, idempotency_key: idempotencyKey };
      const spec = lookupOperation(withKey.operation);
      if (spec === null || !spec.consequential) {
        return this.refusedBeforeAdmission(withKey, idempotencyKey, {
          code: spec === null ? "unsupported_operation" : "not_action_admitted",
          message:
            spec === null
              ? `operation '${withKey.operation}' is not a canonical operation of this surface`
              : `operation '${withKey.operation}' is registered but read-shaped; it is served by the canonical read path, not the action admission path`,
          reasonCodes: spec === null ? ["unsupported_operation"] : ["not_action_admitted"],
          descriptor: null,
        });
      }
      const digest = payloadDigest(withKey.payload);
      const knownDigest = this.keyLedger.get(idempotencyKey);
      if (knownDigest === undefined) {
        return this.refusedBeforeAdmission(withKey, idempotencyKey, {
          code: "unknown_idempotency_key",
          message: `idempotency key '${idempotencyKey}' was never issued by this gateway`,
          reasonCodes: ["unknown_idempotency_key"],
          descriptor: null,
        });
      }
      if (knownDigest !== digest) {
        return this.refusedBeforeAdmission(withKey, idempotencyKey, {
          code: "idempotency_key_payload_mismatch",
          message: `idempotency key '${idempotencyKey}' was issued for a different payload`,
          reasonCodes: ["idempotency_key_payload_mismatch"],
          descriptor: null,
        });
      }
      if (!this.unknownOutcomes.has(idempotencyKey)) {
        return this.refusedBeforeAdmission(withKey, idempotencyKey, {
          code: "outcome_not_unknown",
          message: `idempotency key '${idempotencyKey}' has a known outcome; reconciliation is not applicable`,
          reasonCodes: ["outcome_not_unknown"],
          descriptor: null,
        });
      }
      return this.dispatch(withKey, this.route);
    });
  }

  private admit(operation: OperationId, key: string, outcome: CanonicalOutcome): GatewayOutcome {
    if (outcome.kind === "accepted") {
      return Object.freeze({
        class: "accepted" as const,
        operation,
        idempotency_key: key,
        run_id: outcome.run_id,
        unit_id: outcome.unit_id,
        tickets_resolved: outcome.tickets_resolved,
        committed_now: outcome.committed_now,
      });
    }
    if (outcome.kind === "refused") {
      return Object.freeze({
        class: "refused" as const,
        operation,
        idempotency_key: key,
        code: outcome.code,
        message: outcome.message,
        reason_codes: Object.freeze([outcome.code]),
        stale: false,
        descriptor: null,
      });
    }
    return Object.freeze({
      class: "failed" as const,
      operation,
      idempotency_key: key,
      code: outcome.code,
      message: outcome.message,
    });
  }

  private refusedBeforeAdmission(
    request: OperationInvocationRequest,
    key: string,
    denial: {
      code: string;
      message: string;
      reasonCodes: readonly string[];
      descriptor: AffordanceDescriptor | null;
    },
  ): GatewayOutcome {
    return Object.freeze({
      class: "refused" as const,
      operation: request.operation,
      // The minted key is surfaced, never hidden: it is deterministic (the
      // verifier recomputed it), so secrecy was never a defense — the fresh
      // gate and observability enforcement are (Correction Round 1 BLOCKING 2).
      idempotency_key: request.idempotency_key ?? key,
      code: denial.code,
      message: denial.message,
      reason_codes: Object.freeze([...denial.reasonCodes]),
      stale: true,
      descriptor: denial.descriptor,
    });
  }
}

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * The HTTP surface transport: the workbench's production route to canonical
 * admission over `gauntlet serve` (T00A/T01's seven-route JSON surface).
 * Wire conventions mirror crates/adapters/gauntlet-adapter-surface-json:
 * accepted → 200 {action, run_id, unit_id, committed_now, tickets_resolved};
 * every error — typed refusal (409), session auth (401/403), malformed
 * request (400), infrastructure failure (5xx) — carries the NESTED body
 * {"error":{"code","message"}} (lib.rs:205-213 `error_json`). A flat
 * top-level {code,message} stays tolerated for resilience (non-Rust
 * stand-ins); the nested shape wins. A transport-level loss (fetch
 * rejection) THROWS so the gateway resolves outcome_unknown.
 */
export class HttpOperatorActionRoute implements OperatorActionRoute {
  private readonly baseUrl: string;

  constructor(bindAddress = "127.0.0.1:4000") {
    const url =
      bindAddress.startsWith("http://") || bindAddress.startsWith("https://")
        ? bindAddress
        : `http://${bindAddress}`;
    this.baseUrl = url.replace(/\/+$/, "");
  }

  async observe(_target: { unit: string | null }): Promise<AffordanceObservation> {
    const response = await fetch(`${this.baseUrl}/api/v1/state`, {
      signal: AbortSignal.timeout(5000),
      method: "GET",
      headers: { Accept: "application/json" },
    });
    if (!response.ok) {
      // Fail closed on EVERY transport (Correction Round 1 BLOCKING 2): an
      // unreadable surface THROWS, so the gateway's typed
      // `revalidation_unavailable` refusal fires for fresh invocations,
      // §16.3 re-drives, and reconciliation alike — never a non-throwing
      // "unreachable" observation that a re-drive could route past.
      throw new Error(`the canonical surface state read failed (HTTP ${response.status})`);
    }
    const data = (await response.json()) as Record<string, unknown>;
    if (!Array.isArray(data["units"])) throw new Error("malformed canonical state");
    const unitsRaw = Array.isArray(data["units"]) ? data["units"] : [];
    // W3 (wire honesty): only escalations the backend actually SERVED surface
    // here. The real Rust /state body (snapshot_json,
    // crates/adapters/gauntlet-adapter-surface-json/src/lib.rs:94-150) serves
    // run, escalated, in_flight, units {id,status,tier}, budget,
    // sandbox_denials (a denial COUNT the surface serves — never a containment
    // claim), open_residuals, run_derivation — `escalated` carries BARE ticket
    // ids with NO unit binding, and no `open_escalations` object list exists.
    // A null-unit ticket matches EVERY unit in the affordance resolver, so
    // fabricating one from `escalated` ids made unit.resolve over-available
    // whenever any ticket existed; an absent `open_escalations` field is
    // honest absence — [] — never a fabricated ticket (entries a surface DOES
    // serve, including genuinely unbound unit_id:null ones, map verbatim).
    const escalationsRaw = Array.isArray(data["open_escalations"]) ? data["open_escalations"] : [];
    const openEscalations = escalationsRaw.map((entry) => {
      const record = entry as Record<string, unknown>;
      return {
        ticket_id: String(record["ticket_id"] ?? ""),
        unit_id: typeof record["unit_id"] === "string" ? record["unit_id"] : null,
      };
    });
    // The real surface's budget fact is served under `budget` with a different
    // member set (lib.rs:169-199 budget_json: rounds/parallel/depth/
    // wall_seconds/tokens/cost_usd_micro) — it is NOT conflated into this
    // projection's budget_remaining shape; an absent field stays absent.
    const budgetRaw = data["budget_remaining"] as Record<string, unknown> | null | undefined;
    return Object.freeze({
      reachable: true,
      degraded: false,
      units: Object.freeze(
        unitsRaw.map((entry) => {
          if (typeof entry !== "object" || entry === null)
            throw new Error("malformed canonical unit");
          const record = entry as Record<string, unknown>;
          const id = record["id"] ?? record["unit_id"];
          if (typeof id !== "string" || !id || typeof record["status"] !== "string")
            throw new Error("malformed canonical unit");
          return {
            unit_id: id,
            status: record["status"],
          };
        }),
      ),
      open_escalations: Object.freeze(openEscalations),
      budget_remaining:
        budgetRaw && typeof budgetRaw === "object"
          ? Object.freeze({
              usd_micro: Number(budgetRaw["usd_micro"] ?? 0),
              tokens: Number(budgetRaw["tokens"] ?? 0),
            })
          : null,
      governance_denied_units: Object.freeze(stringList(data["governance_denied_units"])),
      authority_revoked_units: Object.freeze(stringList(data["authority_revoked_units"])),
      capabilities: Object.freeze([]),
    });
  }

  async perform(command: CanonicalActionCommand): Promise<CanonicalOutcome> {
    const headers: Record<string, string> = {
      Accept: "application/json",
    };
    if (command.operator.length > 0) {
      headers["X-Gauntlet-Operator"] = command.operator;
    }
    if (command.session_token.length > 0) {
      headers["Authorization"] = `Bearer ${command.session_token}`;
    }
    // W2 (wire honesty): the Rust action path accepts ONLY an empty body or a
    // JSON object whose "note" member is a string
    // (crates/adapters/gauntlet-adapter-surface-json/src/http.rs:494-541 — a
    // non-empty body is parsed as JSON; a non-object body or non-string note
    // is a typed 400 body_unparsable). The note therefore rides as
    // {"note":"..."} with content-type application/json; an empty note sends
    // NO body (http.rs:497-498: an empty trimmed body IS the empty note).
    const requestBody =
      command.note.length === 0 ? undefined : JSON.stringify({ note: command.note });
    if (requestBody !== undefined) {
      headers["Content-Type"] = "application/json";
    }
    // The canonical precondition anchor (?run=) rides the query when bound.
    const query = command.run === null ? "" : `?run=${encodeURIComponent(command.run)}`;
    const response = await fetch(
      `${this.baseUrl}/api/v1/units/${encodeURIComponent(command.unit)}/${command.action_kind}${query}`,
      {
        method: "POST",
        signal: AbortSignal.timeout(5000),
        headers,
        body: requestBody,
      },
    );
    let body: Record<string, unknown> = {};
    try {
      body = (await response.json()) as Record<string, unknown>;
    } catch {
      // Non-JSON body: the status code alone classifies the outcome below.
    }
    if (response.ok) {
      // Require the canonical acceptance facts (Correction Round 1 MINOR 1):
      // a bare 2xx is NEVER fabricated into an acceptance, and committed_now
      // is never defaulted — a body missing run_id/unit_id/committed_now is
      // a failed outcome (fail-closed).
      const runId = body["run_id"];
      const unitId = body["unit_id"];
      const committedNow = body["committed_now"];
      if (
        typeof runId !== "string" ||
        runId.length === 0 ||
        typeof unitId !== "string" ||
        unitId.length === 0 ||
        typeof committedNow !== "boolean"
      ) {
        return {
          kind: "failed",
          code: "malformed_acceptance",
          message: `the surface answered HTTP ${response.status} without the canonical acceptance facts (run_id, unit_id, committed_now); the gateway refuses to fabricate them`,
        };
      }
      return {
        kind: "accepted",
        run_id: runId,
        unit_id: unitId,
        tickets_resolved: stringList(body["tickets_resolved"]),
        committed_now: committedNow,
      };
    }
    // W1 (wire honesty): the Rust surface ALWAYS nests its typed error facts —
    // {"error":{"code","message"}} (lib.rs:205-213 `error_json`; every error
    // body http.rs serves is built from it). A flat top-level {code,message}
    // stays tolerated for resilience against non-Rust stand-ins; the NESTED
    // shape wins whenever it is present, so the canonical refusal/failure
    // code and message flow through exactly (the gateway's "refusal code
    // preserved" claim above).
    const errorRecord =
      typeof body["error"] === "object" && body["error"] !== null
        ? (body["error"] as Record<string, unknown>)
        : body;
    const code = typeof errorRecord["code"] === "string" ? errorRecord["code"] : "surface_error";
    const message = typeof errorRecord["message"] === "string" ? errorRecord["message"] : "";
    if (response.status >= 500) {
      return { kind: "failed", code, message };
    }
    return { kind: "refused", code, message };
  }
}

function stringList(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((entry): entry is string => typeof entry === "string")
    : [];
}
