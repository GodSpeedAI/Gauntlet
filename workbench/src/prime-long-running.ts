/**
 * Long-running Prime strategy mapping: mailbox delivery as operator choices,
 * execution substrates as provenance-only records, and detach-without-cancel
 * semantics (plan T10, REQ-PRIME-003, REQ-PRIME-005, REQ-PRIME-008..010,
 * REQ-GOAL-003; prereg intervention class A3, Attack 2).
 *
 * Core Invariants:
 *   - Mailbox is COMMUNICATION, never completion (REQ-PRIME-005): every operator
 *     delivery plan carries the literal type `establishes_completion: false`, so
 *     no code path can construct a delivery that settles. A message saying "done"
 *     before a schema-valid result contract leaves the child NON-complete.
 *   - Heartbeats, goals, schedules, autonomous mode, compaction, refinement, and
 *     harness state are execution substrates/provenance ONLY (REQ-PRIME-008). The
 *     installed Prime adapter model (prime-host.ts) exposes NONE of them, so
 *     `requestSubstrateStrategy` refuses each with a typed unsupported outcome —
 *     recorded unsupported, never faked. The `PrimeSubstrateRecord` type pins the
 *     contract for any future supported adapter: provenance records whose
 *     `canonical_direction`/`canonical_settlement`/`canonical_capability` fields
 *     are LITERALLY `false` at the type level.
 *   - Liveness never implies success: completion status is derived ONLY from a
 *     schema-valid result contract; handle status and mailbox volume ride along
 *     as recorded signals, never as a basis.
 *   - Detach is not cancel (REQ-PRIME-009, Attack 2): the TUI host's detach
 *     performs read-only projection and issues NO cancel/stop (the receipt's
 *     `cancel_calls_issued` is the literal type `0`), and the host holds no
 *     termination capability at all. Authorized work outlives the host: the child
 *     lifecycle lives in the Prime adapter (the Prime-owned model), and a fresh
 *     host reattaches from qualified Prime records — the adapter's projected
 *     session records — never from a prior host's memory (REQ-PRIME-010).
 *   - No parallel lifecycle model: this module never spawns, advances, or
 *     terminates a Prime session. Spawning happens ONLY through the gateway-
 *     gated delegation path (prime-recursion); lifecycle changes come only from
 *     Prime-side facts (T04's adapter contracts).
 */

import type { PrimeHostPort, PrimeMailboxMessage, PrimeSessionState } from "./prime-host";
import type { PrimeDelegationDispatcher } from "./prime-recursion";
import { projectHostToSessionRecords } from "./prime-recursion";
import type { PrimeTopologyNode } from "./prime-topology";
import { reconstructTopology } from "./prime-topology";
import type { SubstrateProvenanceRow } from "./substrate-inventory";
import { substrateProvenanceMap } from "./substrate-inventory";

// --- Mailbox delivery as semantic operator choices (REQ-PRIME-005) ------------

/**
 * The semantic operator choice a mailbox delivery expresses: a follow-up adds
 * information to running work; a steer redirects running work. Both are
 * communication — neither is ever a completion, settlement, or capability claim.
 */
export type MailboxOperatorIntent = "follow_up" | "steer";

/** The operator-choice plan for one mailbox delivery. */
export interface OperatorMessagePlan {
  readonly intent: MailboxOperatorIntent;
  /** The semantic operator choice (plan T10: follow-up vs steer). */
  readonly semantic: MailboxOperatorIntent;
  /** LITERAL false at the type level: a mailbox delivery can never establish completion. */
  readonly establishes_completion: false;
  /**
   * Honest substrate note: the installed Prime adapter model exposes no
   * deferred/queued delivery-mode control, so delivery is immediate
   * communication through the T04 mailbox contract.
   */
  readonly delivery_mode_control: "unsupported_delivered_immediately";
}

/** The child's completion status, derived ONLY from the result contract. */
export interface ChildCompletionStatus {
  readonly session_id: string;
  readonly complete: boolean;
  /** The ONLY basis on which `complete` can be true. */
  readonly basis: "schema_valid_result_contract" | "none";
  /** Liveness/communication signals — recorded provenance, never a basis. */
  readonly liveness_signals: {
    readonly handle_status: string;
    readonly mailbox_messages: number;
  };
}

/**
 * Derive a child's completion status. `complete` is true ONLY when a schema-valid
 * result contract exists (T04's completion contract). Handle status ("running" —
 * liveness) and mailbox volume are recorded as signals and contribute nothing.
 */
export function completionStatus(session: PrimeSessionState): ChildCompletionStatus {
  const complete = session.resultContract?.schema_valid === true;
  return Object.freeze({
    session_id: session.handle.session_id,
    complete,
    basis: complete ? "schema_valid_result_contract" : "none",
    liveness_signals: Object.freeze({
      handle_status: session.handle.status,
      mailbox_messages: session.messages.length,
    }),
  });
}

/** Typed messaging failure carrying the delivery locus. */
export class PrimeMessagingError extends Error {
  constructor(detail: string) {
    super(`prime operator messaging: ${detail}`);
    this.name = "PrimeMessagingError";
  }
}

/** Map an operator intent to its delivery plan (pure, typed, non-settling). */
export function mapDeliveryIntent(intent: MailboxOperatorIntent): OperatorMessagePlan {
  return Object.freeze({
    intent,
    semantic: intent,
    establishes_completion: false,
    delivery_mode_control: "unsupported_delivered_immediately" as const,
  });
}

export interface DeliveredOperatorMessage {
  /** The (non-settling) plan the delivery executed. */
  readonly plan: OperatorMessagePlan;
  /** The recorded mailbox message (communication provenance). */
  readonly message: PrimeMailboxMessage;
  /** The child's completion status AFTER delivery — unchanged by any message. */
  readonly child_completion_after: ChildCompletionStatus;
}

export interface DeliverMessageOptions {
  readonly session_id: string;
  readonly intent: MailboxOperatorIntent;
  readonly content: string;
  readonly sender?: string | undefined;
}

/**
 * Deliver one operator message through the adapter's mailbox as COMMUNICATION
 * (kind "communication" even for steer — steering is communication, never a
 * control-plane completion). The child's status is untouched: a message
 * containing "done" before a valid result leaves the child NON-complete.
 */
export function deliverOperatorMessage(
  adapter: PrimeHostPort,
  options: DeliverMessageOptions,
): DeliveredOperatorMessage {
  const plan = mapDeliveryIntent(options.intent);
  if (options.content.trim().length === 0) {
    throw new PrimeMessagingError("message content must be a non-empty string");
  }
  let message: PrimeMailboxMessage;
  try {
    message = adapter.receiveMailboxMessage(options.session_id, {
      sender: options.sender ?? "operator",
      recipient: options.session_id,
      content: options.content,
      kind: "communication",
    });
  } catch (error) {
    throw new PrimeMessagingError(
      `delivery to '${options.session_id}' failed: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  const session = adapter.getSession(options.session_id);
  if (session === null) {
    throw new PrimeMessagingError(`session '${options.session_id}' vanished during delivery`);
  }
  return Object.freeze({
    plan,
    message,
    child_completion_after: completionStatus(session),
  });
}

// --- Execution substrates as provenance-only records (REQ-PRIME-008) ----------

/** The long-running Prime controls REQ-PRIME-008 names. */
export type PrimeSubstrateKind =
  | "heartbeat"
  | "goal"
  | "schedule"
  | "autonomous_mode"
  | "compaction"
  | "refinement"
  | "harness_state";

export const PRIME_SUBSTRATE_KINDS: readonly PrimeSubstrateKind[] = Object.freeze([
  "heartbeat",
  "goal",
  "schedule",
  "autonomous_mode",
  "compaction",
  "refinement",
  "harness_state",
]);

/**
 * The record shape a SUPPORTED substrate would produce. Deliberately provenance-
 * only: `canonical_direction`, `canonical_settlement`, and `canonical_capability`
 * are LITERAL false at the type level, so a substrate record can never be
 * construed as Gauntlet direction, settlement, or capability truth. No factory
 * exists in this module because the installed adapter exposes no substrate API —
 * the type pins the contract for the task that adds one.
 */
export interface PrimeSubstrateRecord {
  readonly substrate: PrimeSubstrateKind;
  readonly session_id: string;
  readonly detail: string;
  /** LITERAL true: this is execution provenance. */
  readonly substrate_provenance: true;
  /** LITERAL false: never Gauntlet direction. */
  readonly canonical_direction: false;
  /** LITERAL false: never Gauntlet settlement. */
  readonly canonical_settlement: false;
  /** LITERAL false: never a Gauntlet capability claim. */
  readonly canonical_capability: false;
  readonly observed_at: string;
}

/** One honest capability fact about the installed adapter model. */
export interface PrimeSubstrateCapabilityFact {
  readonly substrate: PrimeSubstrateKind;
  readonly supported: false;
  readonly reason: "not_exposed_by_installed_prime_adapter_model";
}

/**
 * The substrate inventory: what the INSTALLED Prime adapter model actually
 * exposes. Today that is none of the REQ-PRIME-008 controls — recorded
 * unsupported, never faked (hard constraint).
 */
export function substrateInventory(): readonly PrimeSubstrateCapabilityFact[] {
  return PRIME_SUBSTRATE_KINDS.map((substrate) =>
    Object.freeze({
      substrate,
      supported: false as const,
      reason: "not_exposed_by_installed_prime_adapter_model" as const,
    }),
  );
}

export type SubstrateStrategyOutcome = {
  readonly class: "unsupported_substrate";
  readonly substrate: PrimeSubstrateKind;
  readonly session_id: string;
  /** The honest absence reason; the substrate is never simulated. */
  readonly reason: "not_exposed_by_installed_prime_adapter_model";
};

/**
 * RE-QUALIFIED substrate strategy refusal (T24): same typed unsupported
 * outcome as `requestSubstrateStrategy`, carrying the feature's typed
 * three-way provenance row (real binary / adapter model / workbench) from
 * substrate-inventory.ts. The refusal itself is unchanged — the ADAPTER MODEL
 * still exposes no substrate controls, so the strategy is still refused, never
 * simulated; what is new is that the refusal now cites the re-qualified
 * provenance map instead of leaving the real binary's surface unrecorded
 * (T10 trigger-record MAJOR 2 discharge, REQ-PRIME-008).
 */
export type RequalifiedSubstrateOutcome = SubstrateStrategyOutcome & {
  /** The feature's three-way provenance row (canonical fields LITERAL false). */
  readonly provenance: SubstrateProvenanceRow;
};

/**
 * Attempt to map an admitted Gauntlet strategy onto a long-running Prime
 * control, WITH the T24 three-way provenance attached. Every REQ-PRIME-008
 * substrate is still refused with a typed unsupported outcome — the adapter
 * model exposes none of them and T24's requalification did not change the
 * adapter model — but the outcome now names exactly which layer exposes what,
 * so the real binary's surface is recorded without being promoted into the
 * model or the workbench path.
 */
export function requestSubstrateStrategyWithProvenance(
  substrate: PrimeSubstrateKind,
  sessionId: string,
): RequalifiedSubstrateOutcome {
  const row = substrateProvenanceMap().find(
    (candidate) => candidate.req_prime_008_class === substrate,
  );
  if (row === undefined) {
    throw new Error(
      `prime substrate requalification: REQ-PRIME-008 substrate '${substrate}' has no provenance row; the map is incomplete`,
    );
  }
  return Object.freeze({
    class: "unsupported_substrate" as const,
    substrate,
    session_id: sessionId,
    reason: "not_exposed_by_installed_prime_adapter_model" as const,
    provenance: row,
  });
}

/**
 * Attempt to map an admitted Gauntlet strategy onto a long-running Prime
 * control. Every REQ-PRIME-008 substrate is refused with a typed unsupported
 * outcome because the installed adapter model exposes none of them — the
 * strategy is recorded unsupported, never simulated (REQ-GOAL-003: exploit the
 * substrate where it exists; represent absence honestly where it does not).
 */
export function requestSubstrateStrategy(
  substrate: PrimeSubstrateKind,
  sessionId: string,
): SubstrateStrategyOutcome {
  return Object.freeze({
    class: "unsupported_substrate" as const,
    substrate,
    session_id: sessionId,
    reason: "not_exposed_by_installed_prime_adapter_model" as const,
  });
}

// --- Detach semantics: no implicit cancellation (REQ-PRIME-009) ---------------

/** Typed receipt for a TUI detach. `cancel_calls_issued` is the literal type 0. */
export interface DetachReceipt {
  readonly detached: true;
  /** LITERAL 0 at the type level: detach issues no cancel/stop — there is no such call to make. */
  readonly cancel_calls_issued: 0;
  /** Real adapter sessions still authorized after the detach. */
  readonly sessions_left_authorized: readonly string[];
  /** The qualified source the authorization snapshot was projected from. */
  readonly source: "qualified_prime_adapter_records";
}

/**
 * A call-logging wrapper around a Prime adapter: every method invocation is
 * recorded by name, so tests can PROVE a host operation (detach/close) issues no
 * cancel/stop against the adapter (the plan's "assert on the adapter call log").
 */
export function loggingPrimeAdapter(adapter: PrimeHostPort): {
  readonly logged: PrimeHostPort;
  readonly calls: readonly string[];
} {
  const calls: string[] = [];
  const logged = new Proxy(adapter, {
    get(target, property, receiver) {
      const value = Reflect.get(target, property, receiver);
      if (typeof property === "string" && typeof value === "function") {
        return (...args: readonly unknown[]) => {
          calls.push(property);
          return (value as (...fnArgs: unknown[]) => unknown).apply(target, [...args]);
        };
      }
      return value;
    },
  });
  return {
    logged: logged as PrimeHostPort,
    // The LIVE log (callers observe it after operations complete); typed
    // read-only so callers cannot mutate it.
    calls,
  };
}

/** Typed host failure carrying the host-operation locus. */
export class PrimeHostLifecycleError extends Error {
  constructor(operation: string, detail: string) {
    super(`prime workbench host ${operation}: ${detail}`);
    this.name = "PrimeHostLifecycleError";
  }
}

export interface PrimeWorkbenchHostOptions {
  /** The Prime-side session model (T04 adapter). The host is a disposable viewer of it. */
  readonly adapter: PrimeHostPort;
  /** The gateway-gated delegation dispatcher, when the host delegates work. */
  readonly delegation?: PrimeDelegationDispatcher | undefined;
}

/**
 * The workbench-side TUI host: a DISPOSABLE composition over the Prime adapter.
 * It owns no session lifecycle — closing or detaching the host cannot terminate
 * authorized work, because it has no termination capability to begin with
 * (REQ-PRIME-009). Children keep living in the adapter; a fresh host reattaches
 * from qualified Prime records (`reattachToPrime`).
 */
export class PrimeWorkbenchHost {
  readonly adapter: PrimeHostPort;
  private delegationDispatcher: PrimeDelegationDispatcher | undefined;
  private closed = false;

  constructor(options: PrimeWorkbenchHostOptions) {
    this.adapter = options.adapter;
    this.delegationDispatcher = options.delegation;
  }

  get delegation(): PrimeDelegationDispatcher | undefined {
    return this.delegationDispatcher;
  }

  isClosed(): boolean {
    return this.closed;
  }

  /**
   * Deliver an operator message through the mailbox (follow-up/steer). Refused
   * once the host is closed — messaging is a host capability, lifecycle is not.
   */
  messageChild(options: DeliverMessageOptions): DeliveredOperatorMessage {
    if (this.closed) {
      throw new PrimeHostLifecycleError("messageChild", "the host is closed");
    }
    return deliverOperatorMessage(this.adapter, options);
  }

  /**
   * TUI detach (REQ-PRIME-009): read-only projection of the authorized work from
   * qualified Prime records. Sends NO cancel/stop — the adapter is not asked to
   * terminate anything (no such adapter capability exists, and none is simulated);
   * authorized work continues in Prime. The optional argument accepts a call-
   * logging adapter wrapper so tests can verify the absence on the call log.
   */
  detach(observedAdapter: PrimeHostPort = this.adapter): DetachReceipt {
    const records = projectHostToSessionRecords(observedAdapter);
    return Object.freeze({
      detached: true as const,
      cancel_calls_issued: 0 as const,
      sessions_left_authorized: Object.freeze(
        records
          .filter((record) => record.status === "admitted" || record.status === "running")
          .map((record) => record.sessionId),
      ),
      source: "qualified_prime_adapter_records" as const,
    });
  }

  /**
   * Close the host object (the TUI going away). Drops host-local references
   * only: it performs NO adapter calls of any kind, so Prime-side work is
   * untouched. The host is unusable afterwards; the adapter is not.
   */
  close(): void {
    this.closed = true;
    this.delegationDispatcher = undefined;
  }
}

/** Receipt from reattaching a fresh host to real Prime state. */
export interface ReattachmentReceipt {
  /** The REAL parent/child topology from qualified Prime records (no synthesis). */
  readonly topology: readonly PrimeTopologyNode[];
  /** Sessions the records show as admitted/running — still authorized. */
  readonly running_sessions: readonly string[];
  /** Sessions the records show as completed — completion is the record's, never inferred. */
  readonly completed_sessions: readonly string[];
  /** The qualified source identity (REQ-PRIME-010): adapter records, not host memory. */
  readonly source: "qualified_prime_adapter_records";
}

/**
 * Reattach to real Prime state from qualified Prime/Gauntlet records: the
 * adapter's projected session records reconstructed through T04's topology
 * projection. Never reads a prior host's memory; never infers status from
 * messages, liveness, or session-directory presence (REQ-PRIME-010).
 */
export function reattachToPrime(adapter: PrimeHostPort): ReattachmentReceipt {
  const records = projectHostToSessionRecords(adapter);
  const topology = reconstructTopology(records);
  return Object.freeze({
    topology,
    running_sessions: Object.freeze(
      records
        .filter((record) => record.status === "admitted" || record.status === "running")
        .map((record) => record.sessionId),
    ),
    completed_sessions: Object.freeze(
      records.filter((record) => record.status === "completed").map((record) => record.sessionId),
    ),
    source: "qualified_prime_adapter_records" as const,
  });
}

/**
 * The honest operator-termination inventory: the canonical surface serves NO
 * cancellation/stop operation (REQ-OPS-003 — launch, steering, cancellation are
 * absent from the registry and render unsupported, never a fallback). Until a
 * canonical stop exists, NO workbench path — detach, host close, or messaging —
 * terminates Prime work; lifecycle changes come only from Prime-side facts.
 */
export function canonicalStopAvailability(): {
  readonly registered: false;
  readonly detail: string;
} {
  return Object.freeze({
    registered: false as const,
    detail:
      "the canonical surface serves no cancellation/stop operation (REQ-OPS-003); absent operations render unsupported — an explicit canonical stop, if one is ever served, would be the only operator termination path",
  });
}
