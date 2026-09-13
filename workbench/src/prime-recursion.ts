/**
 * Recursive Prime delegation over the T04 adapter's real session model (plan T10,
 * REQ-OPS-001, REQ-PRIME-001..005, REQ-GOAL-003; prereg intervention class A2).
 *
 * Core Invariants:
 *   - Delegation routes through the ONE canonical admission path: every drive is a
 *     T09 `OperationGateway` drive over an `OperatorActionRoute` whose `perform`
 *     FIRST forwards the canonical command to the canonical admission route and
 *     only on an ACCEPTED outcome admits the Prime child. Canonical dispatch
 *     ownership therefore precedes process start (REQ-OPS-001), and a denied or
 *     stale operation produces the SAME typed gateway denial with ZERO Prime side
 *     effect — no child is ever spawned behind a denial.
 *   - Admission is nonblocking (REQ-PRIME-003): the spawn/admission handle is
 *     returned immediately and is never presented as the child result; completion
 *     is observed independently through T04's result-contract contract.
 *   - Topology is real (REQ-PRIME-001): parent/child identities come from the
 *     adapter's actual sessions; this module mints no session lifecycle and no
 *     synthetic identity. Depth is computed by walking the adapter's real parent
 *     chains, never from workbench-local bookkeeping.
 *   - Bounds are workbench policy, honestly: the installed Prime adapter model
 *     (prime-host.ts) exposes NO native nested-delegation depth or child-budget
 *     capability. `RecursionPolicy` is therefore enforced HERE as a typed
 *     pre-delegation refusal — it is never presented as a Prime capability, and
 *     `adapterCapabilityFacts` records the absence as unsupported rather than
 *     faking it (REQ-GOAL-003: substrates are exploited, never reimplemented or
 *     invented).
 *   - Deterministic death between canonical admission and child spawn is
 *     recoverable with no orphan: a lost response resolves the T09 typed
 *     `outcome_unknown` under the SAME idempotency key, and `recoverDelegation`
 *     re-drives THAT key with the SAME payload — the §16.3 idempotent re-entry
 *     commits nothing twice and the child spawns exactly once. A host death after
 *     spawn but before bookkeeping leaves the REAL child recoverable from the
 *     adapter's topology (`reconcileLedger`), never re-minted and never killed.
 */

import type { AffordanceObservation } from "./affordance";
import { FocusRef } from "./focus";
import type {
  CanonicalActionCommand,
  CanonicalOutcome,
  GatewayOutcome,
  OperationInvocationRequest,
  OperatorActionRoute,
} from "./operation-gateway";
import { OperationGateway } from "./operation-gateway";
import type { PrimeContextEnvelope } from "./prime-context-envelope";
import { envelopeToJSON, PRIME_CONTEXT_ENVELOPE_SCHEMA } from "./prime-context-envelope";
import type { PrimeAdmissionHandle, PrimeHostPort, PrimeSessionState } from "./prime-host";
import type { PrimeSessionRecord, PrimeTopologyNode } from "./prime-topology";
import { reconstructTopology } from "./prime-topology";

/**
 * Workbench-side recursion bounds. These are POLICY (typed refusals enforced
 * before any drive), not Prime capabilities: the installed adapter model exposes
 * no native depth/budget surface, and this type deliberately claims none.
 */
export interface RecursionPolicy {
  /** Maximum child depth below the root (a root session is depth 0). */
  readonly max_depth: number;
  /** Maximum concurrently admitted children per one parent session. */
  readonly max_children_per_parent: number;
}

export const DEFAULT_RECURSION_POLICY: RecursionPolicy = Object.freeze({
  max_depth: 3,
  max_children_per_parent: 4,
});

/** Typed pre-delegation refusal codes (zero side effect on every code). */
export type RecursionRefusalCode =
  | "envelope_required"
  | "parent_session_unknown"
  | "duplicate_session_id"
  | "depth_exceeded"
  | "child_budget_exhausted"
  | "outcome_not_unknown";

/**
 * Honest capability inventory for the recursion surface: what the INSTALLED
 * Prime adapter model actually exposes. Absence is recorded unsupported —
 * never faked (REQ-GOAL-003, hard constraint: features absent from the adapter
 * model are recorded unsupported).
 */
export interface DelegationCapabilityFact {
  readonly capability: string;
  readonly supported: boolean;
  /** For unsupported entries: the honest absence reason. */
  readonly reason: string | null;
}

/** What the recursion layer can honestly claim about nested delegation control. */
export function delegationCapabilityFacts(): readonly DelegationCapabilityFact[] {
  return Object.freeze([
    Object.freeze({
      capability: "child_session_admission",
      supported: true,
      reason: null,
    }),
    Object.freeze({
      capability: "mailbox_communication",
      supported: true,
      reason: null,
    }),
    Object.freeze({
      capability: "result_contract_completion",
      supported: true,
      reason: null,
    }),
    Object.freeze({
      capability: "native_nested_delegation_depth_limit",
      supported: false,
      reason: "not_exposed_by_installed_prime_adapter_model",
    }),
    Object.freeze({
      capability: "native_child_budget_accounting",
      supported: false,
      reason: "not_exposed_by_installed_prime_adapter_model",
    }),
  ]);
}

/** One recursive delegation request: a canonical operation delegated to a Prime child. */
export interface PrimeDelegationRequest {
  /** The canonical operation invocation (registry vocabulary; gateway-gated). */
  readonly operation_request: OperationInvocationRequest;
  /** The bounded context envelope the child receives (required — there is no default envelope). */
  readonly envelope: PrimeContextEnvelope;
  /** The child session id — supplied, then owned by the adapter; never minted here. */
  readonly child_session_id: string;
  /** Real adapter session id of the parent, or null for a root-level delegation. */
  readonly parent_session_id: string | null;
  /** The task instruction carried with the envelope as the child's received context. */
  readonly task: string;
  readonly contract_path?: string | undefined;
  /**
   * Work-mode passthrough to `PrimeHostPort.spawnChild` (plan P1): a
   * composer-launched child runs WITH tools enabled at a real project `cwd`.
   * Absent (every pre-P1 caller) means the adapter's own default — for
   * `RealPrimeHost`, `--no-tools` at the isolated session/agent cwd,
   * completely unchanged.
   */
  readonly work_mode?: { readonly toolsEnabled: boolean; readonly cwd: string } | undefined;
}

/** The workbench delegation record: canonical receipt bound to the real Prime child. */
export interface DelegationRecord {
  readonly delegation_id: string;
  /** Canonical registry operation id (e.g. unit.resolve). */
  readonly operation: string;
  readonly target_unit: string;
  /** The gateway idempotency key of the canonical drive that admitted the child. */
  readonly idempotency_key: string;
  /** The canonical accepted receipt's durable invocation identity (REQ-OPS-001). */
  readonly canonical_run_id: string;
  readonly session_id: string;
  readonly parent_session_id: string | null;
  readonly envelope_id: string;
  /** Degradation disclosure provenance: a whole-case envelope delegation is recorded as such. */
  readonly envelope_kind: "bounded" | "whole_case_disclosed";
  readonly contract_path: string;
  readonly admitted_at: string;
}

export type PrimeDelegationOutcome =
  | {
      readonly class: "admitted";
      readonly delegation_id: string;
      /** Nonblocking admission handle — NEVER the child result or completion (REQ-PRIME-003). */
      readonly handle: PrimeAdmissionHandle;
      readonly record: DelegationRecord;
      /** The canonical accepted outcome (the durable invocation receipt). */
      readonly gateway: GatewayOutcome & { readonly class: "accepted" };
    }
  | {
      readonly class: "refused_by_policy";
      readonly code: RecursionRefusalCode;
      readonly detail: string;
    }
  | {
      /** The SAME typed denial any route receives from the ONE admission path. */
      readonly class: "refused_by_gateway";
      readonly gateway: GatewayOutcome;
    };

export interface DelegationRecoveryReport {
  /** The adapter's REAL topology (Prime-owned identities, projected, not synthesized). */
  readonly real_topology: readonly PrimeTopologyNode[];
  /** Ledger records whose session still exists in the adapter. */
  readonly mapped_delegations: readonly string[];
  /** Ledger records with NO adapter session (death between admission and spawn left nothing half-made). */
  readonly unspawned_delegations: readonly string[];
  /**
   * REAL adapter sessions with no ledger record (death after spawn, record lost
   * with the host): recovered by their actual Prime identity — never re-minted,
   * never implicitly cancelled.
   */
  readonly unmapped_children: readonly PrimeTopologyNode[];
}

export interface PrimeDispatcherOptions {
  readonly adapter: PrimeHostPort;
  /** The canonical admission route (the ONE write path); delegation wraps it, never replaces it. */
  readonly canonical_route: OperatorActionRoute;
  readonly policy?: RecursionPolicy | undefined;
  /** Base directory for child result-contract paths. */
  readonly contract_base?: string | undefined;
}

const DEFAULT_CONTRACT_BASE = ".agents/reports/gauntlet-tui-prime-workbench/evidence/T10/contracts";

/** Project the adapter's actual sessions into durable T04 session records. */
export function projectHostToSessionRecords(adapter: PrimeHostPort): readonly PrimeSessionRecord[] {
  return adapter.listSessions().map((session: PrimeSessionState) => ({
    sessionId: session.handle.session_id,
    parentId: session.handle.parent_session_id,
    model: session.handle.model,
    status: session.handle.status,
    contractPath: session.handle.contract_path,
    messages: session.messages,
    createdAt: session.handle.admitted_at,
  }));
}

/**
 * Depth of a session in the adapter's REAL parent chains (root = 0), or null when
 * the adapter does not know the session. Never reads workbench bookkeeping.
 */
export function sessionDepth(adapter: PrimeHostPort, sessionId: string): number | null {
  let current: PrimeSessionState | null = adapter.getSession(sessionId);
  if (current === null) {
    return null;
  }
  let depth = 0;
  while (current.handle.parent_session_id !== null) {
    const parent: PrimeSessionState | null = adapter.getSession(current.handle.parent_session_id);
    if (parent === null) {
      // Parent unknown to the adapter: depth beyond this point is not knowable.
      return null;
    }
    depth += 1;
    current = parent;
  }
  return depth;
}

/** Typed refusal code for the spawn-time duplicate guard (Correction Round 1 MINOR 3). */
export class DuplicateSpawnError extends Error {
  constructor(detail: string) {
    super(`prime delegation spawn refused: ${detail}`);
    this.name = "DuplicateSpawnError";
  }
}

/**
 * The delegation route: an OperatorActionRoute that adds Prime child admission
 * BEHIND the canonical admission, never beside it. One instance per drive, built
 * over the dispatcher's inner canonical route; the gateway's own gating
 * (fresh revalidation, fail-closed denial, idempotency binding) applies unchanged.
 */
class PrimeDelegationRoute implements OperatorActionRoute {
  constructor(
    private readonly inner: OperatorActionRoute,
    private readonly adapter: PrimeHostPort,
    private readonly context: {
      readonly envelope: PrimeContextEnvelope;
      readonly child_session_id: string;
      readonly parent_session_id: string | null;
      readonly task: string;
      readonly contract_path: string;
      readonly work_mode?: { readonly toolsEnabled: boolean; readonly cwd: string } | undefined;
    },
    private readonly onAdmitted: (record: DelegationRecord, handle: PrimeAdmissionHandle) => void,
  ) {}

  observe(target: { unit: string | null }): Promise<AffordanceObservation> {
    // Fresh canonical observation through the SAME route the command would take:
    // revalidation is the inner canonical surface's, unchanged.
    return this.inner.observe(target);
  }

  async perform(command: CanonicalActionCommand): Promise<CanonicalOutcome> {
    // 1. Canonical dispatch ownership FIRST (REQ-OPS-001): a denial at the one
    //    admission body returns the canonical refusal with ZERO Prime side effect.
    const outcome = await this.inner.perform(command);
    if (outcome.kind !== "accepted") {
      return outcome;
    }
    // 2. SPAWN-TIME DUPLICATE GUARD (Correction Round 1 MINOR 3): the dispatcher's
    //    serialized policy gate protects one dispatcher, but a racing drive from a
    //    DIFFERENT dispatcher (separate gateway/queue) over the same adapter can
    //    still reach the spawn boundary after another drive spawned the child. An
    //    adapter session is NEVER reset: the racing drive throws a typed error
    //    BEFORE spawnChild — zero Prime mutation, the child and its progress stay
    //    exactly as the winning drive left them. The gateway resolves the lost
    //    drive to typed `outcome_unknown` under its own key, whose recovery is
    //    then refused `duplicate_session_id` (an existing child is never re-minted).
    if (this.adapter.getSession(this.context.child_session_id) !== null) {
      throw new DuplicateSpawnError(
        `spawn-time duplicate guard: child session '${this.context.child_session_id}' already exists in the Prime adapter; the racing drive is refused before spawn and the existing child is never reset`,
      );
    }
    // 3. Containment, then process start: the bounded envelope is delivered to the
    //    child as its received context, and only then is the child admitted.
    const handle = this.adapter.spawnChild({
      sessionId: this.context.child_session_id,
      parentSessionId: this.context.parent_session_id,
      contractPath: this.context.contract_path,
      // Real hosts (RealPrimeHost, plan Phase C2) need the goal text to
      // actually launch a process; the in-memory test double ignores it —
      // the mailbox message below remains the inspectable received-context
      // record either way.
      task: this.context.task,
      workMode: this.context.work_mode,
    });
    // The child's RECEIVED CONTEXT is exactly the envelope plus the task —
    // serialized into the mailbox as communication, inspectable afterwards.
    this.adapter.receiveMailboxMessage(this.context.child_session_id, {
      sender: this.context.parent_session_id ?? "orchestrator",
      recipient: this.context.child_session_id,
      content: JSON.stringify({
        received_context: "prime-context-envelope/1",
        task: this.context.task,
        envelope: JSON.parse(envelopeToJSON(this.context.envelope)),
      }),
      kind: "communication",
    });
    const record: DelegationRecord = {
      delegation_id: `delegation:${command.operation}:${command.unit}:${this.context.child_session_id}`,
      operation: command.operation,
      target_unit: command.unit,
      idempotency_key: command.idempotency_key,
      canonical_run_id: outcome.run_id,
      session_id: this.context.child_session_id,
      parent_session_id: this.context.parent_session_id,
      envelope_id: this.context.envelope.envelope_id,
      envelope_kind: this.context.envelope.kind,
      contract_path: this.context.contract_path,
      admitted_at: handle.admitted_at,
    };
    this.onAdmitted(record, handle);
    return outcome;
  }
}

/**
 * The recursive delegation dispatcher: the workbench-side composition of the T09
 * gateway, the canonical admission route, and the T04 Prime adapter. One per
 * workbench host; it owns NO session lifecycle — the adapter (Prime) does.
 */
export class PrimeDelegationDispatcher {
  private readonly adapter: PrimeHostPort;
  private readonly inner: OperatorActionRoute;
  private readonly policy: RecursionPolicy;
  private readonly contractBase: string;
  private readonly gateway: OperationGateway;
  private readonly records = new Map<string, DelegationRecord>();
  /**
   * Per-host decision serialization (Correction Round 1 MINOR 3): the policy
   * gates + canonical drive of one delegation are atomic with respect to every
   * other delegation on this dispatcher. With the gates OUTSIDE the serialized
   * decision, two concurrent drives with the same child_session_id could both
   * pass the duplicate check before either spawned, and the second spawn would
   * silently reset the child (progress wiped). A rejected job never poisons the
   * tail.
   */
  private decisionTail: Promise<unknown> = Promise.resolve();

  constructor(options: PrimeDispatcherOptions) {
    this.adapter = options.adapter;
    this.inner = options.canonical_route;
    this.policy = options.policy ?? DEFAULT_RECURSION_POLICY;
    this.contractBase = options.contract_base ?? DEFAULT_CONTRACT_BASE;
    this.gateway = new OperationGateway(options.canonical_route);
  }

  /** The delegation records admitted through this dispatcher (read-only view). */
  listRecords(): readonly DelegationRecord[] {
    return [...this.records.values()];
  }

  /**
   * The bound Prime host adapter (plan P1). `prime.message`/`prime.cancel`
   * need `stopChild`/`continueChild`, which are NOT part of the base
   * `PrimeHostPort` five-method surface every delegation drive uses — callers
   * narrow with `isExtendedPrimeHost` (prime-host.ts) before using them.
   */
  get primeHost(): PrimeHostPort {
    return this.adapter;
  }

  /** Look up one delegation record by id. */
  getRecord(delegationId: string): DelegationRecord | null {
    return this.records.get(delegationId) ?? null;
  }

  /** The adapter's REAL parent/child topology, projected without synthesis (REQ-PRIME-001). */
  topology(): readonly PrimeTopologyNode[] {
    return reconstructTopology(projectHostToSessionRecords(this.adapter));
  }

  /**
   * Serialize one delegation's decision (Correction Round 1 MINOR 3): the
   * policy gates run INSIDE the serialized decision, so a concurrent drive with
   * the same child identity observes the first drive's spawn and is refused —
   * a second spawn can never reset an existing child.
   */
  private enqueueDecision<T>(job: () => Promise<T>): Promise<T> {
    const run = this.decisionTail.then(
      () => job(),
      () => job(),
    );
    this.decisionTail = run.catch(() => undefined);
    return run;
  }

  /** Count a parent's directly admitted children from the adapter's real sessions. */
  private childCount(parentSessionId: string | null): number {
    return this.adapter
      .listSessions()
      .filter((session) => session.handle.parent_session_id === parentSessionId).length;
  }

  /** Typed pre-delegation policy gates. Every refusal here has ZERO side effects. */
  private checkPolicy(request: PrimeDelegationRequest): RecursionRefusalCode | null {
    if (request.envelope.schema !== PRIME_CONTEXT_ENVELOPE_SCHEMA) {
      return "envelope_required";
    }
    if (
      request.parent_session_id !== null &&
      this.adapter.getSession(request.parent_session_id) === null
    ) {
      return "parent_session_unknown";
    }
    if (this.adapter.getSession(request.child_session_id) !== null) {
      return "duplicate_session_id";
    }
    const parentDepth =
      request.parent_session_id === null
        ? -1
        : sessionDepth(this.adapter, request.parent_session_id);
    if (parentDepth === null) {
      return "parent_session_unknown";
    }
    if (parentDepth + 1 > this.policy.max_depth) {
      return "depth_exceeded";
    }
    if (this.childCount(request.parent_session_id) >= this.policy.max_children_per_parent) {
      return "child_budget_exhausted";
    }
    return null;
  }

  /**
   * Delegate one canonical operation to a Prime child with a bounded envelope.
   * The whole decision — policy gates INSIDE the serialized decision, then the
   * gateway drive — is atomic with respect to every other delegation on this
   * dispatcher (Correction Round 1 MINOR 3). Policy refusals are typed with
   * zero side effects; a denied/stale operation yields the SAME typed gateway
   * denial with no child admitted; an accepted operation admits the child
   * nonblockingly and returns the admission handle (never a result).
   */
  delegate(request: PrimeDelegationRequest): Promise<PrimeDelegationOutcome> {
    return this.enqueueDecision(() => this.drive(request, null));
  }

  /**
   * Deterministic recovery for a delegation whose canonical admission response was
   * lost between admission and spawn (TUI/Bun death): re-drive the SAME
   * idempotency key with the SAME payload and the SAME delegation context. The
   * gateway's outcome_unknown record authorizes the re-entry; the §16.3 idempotent
   * canonical re-entry commits nothing twice; the child then spawns exactly once.
   * Serialized on the same decision queue as delegate() (Correction Round 1
   * MINOR 3).
   */
  recoverDelegation(
    request: PrimeDelegationRequest,
    idempotencyKey: string,
  ): Promise<PrimeDelegationOutcome> {
    return this.enqueueDecision(async () => {
      if (!this.gateway.isOutcomeUnknown(idempotencyKey)) {
        return Object.freeze({
          class: "refused_by_policy" as const,
          code: "outcome_not_unknown" as const,
          detail: `idempotency key '${idempotencyKey}' has no unknown outcome to recover`,
        });
      }
      return this.drive(request, idempotencyKey);
    });
  }

  /** One serialized drive: policy gates, then the canonical+spawn gateway drive. */
  private async drive(
    request: PrimeDelegationRequest,
    idempotencyKey: string | null,
  ): Promise<PrimeDelegationOutcome> {
    const refusal = this.checkPolicy(request);
    if (refusal !== null) {
      return Object.freeze({
        class: "refused_by_policy" as const,
        code: refusal,
        detail: describeRefusal(refusal, request),
      });
    }
    const route = new PrimeDelegationRoute(
      this.inner,
      this.adapter,
      {
        envelope: request.envelope,
        child_session_id: request.child_session_id,
        parent_session_id: request.parent_session_id,
        task: request.task,
        contract_path:
          request.contract_path ?? `${this.contractBase}/${request.child_session_id}.result.json`,
        work_mode: request.work_mode,
      },
      (record) => {
        this.records.set(record.delegation_id, record);
      },
    );
    const operationRequest =
      idempotencyKey === null
        ? request.operation_request
        : { ...request.operation_request, idempotency_key: idempotencyKey };
    const gateway: GatewayOutcome = await this.gateway.invokeVia(route, operationRequest);
    if (gateway.class !== "accepted") {
      return Object.freeze({ class: "refused_by_gateway" as const, gateway });
    }
    const delegationId = `delegation:${gateway.operation}:${gateway.unit_id}:${request.child_session_id}`;
    const record = this.records.get(delegationId);
    const session = this.adapter.getSession(request.child_session_id);
    if (record === undefined || session === null) {
      // Invariant: an accepted canonical outcome always admitted the child and
      // wrote its record (the route admits before reporting acceptance).
      throw new Error(
        `prime delegation dispatcher: accepted outcome for '${delegationId}' has no delegation record or adapter session`,
      );
    }
    return Object.freeze({
      class: "admitted" as const,
      delegation_id: delegationId,
      handle: { ...session.handle },
      record,
      gateway,
    });
  }

  /**
   * Cross-check the delegation ledger against the adapter's REAL sessions (the
   * recovery walk a fresh host performs). Nothing is created, destroyed, or
   * re-minted here: the report names what canonical reconciliation must re-drive
   * (unspawned) and what real Prime children a fresh host can reattach to
   * (unmapped), leaving the canonical owner in charge (REQ-OPS-001, REQ-PRIME-010).
   */
  reconcileLedger(): DelegationRecoveryReport {
    const realTopology = this.topology();
    const realSessionIds = new Set(
      projectHostToSessionRecords(this.adapter).map((record) => record.sessionId),
    );
    const mapped: string[] = [];
    const unspawned: string[] = [];
    const recordedSessions = new Set<string>();
    for (const record of this.records.values()) {
      recordedSessions.add(record.session_id);
      if (realSessionIds.has(record.session_id)) {
        mapped.push(record.delegation_id);
      } else {
        unspawned.push(record.delegation_id);
      }
    }
    const unmapped = collectNodes(realTopology).filter(
      (node) => !recordedSessions.has(node.sessionId),
    );
    return Object.freeze({
      real_topology: realTopology,
      mapped_delegations: Object.freeze(mapped),
      unspawned_delegations: Object.freeze(unspawned),
      unmapped_children: Object.freeze(unmapped),
    });
  }
}

function collectNodes(nodes: readonly PrimeTopologyNode[]): readonly PrimeTopologyNode[] {
  const out: PrimeTopologyNode[] = [];
  for (const node of nodes) {
    out.push(node);
    out.push(...collectNodes(node.children));
  }
  return out;
}

function describeRefusal(code: RecursionRefusalCode, request: PrimeDelegationRequest): string {
  switch (code) {
    case "envelope_required":
      return "a constructed prime-context-envelope is required for delegation; there is no default envelope";
    case "parent_session_unknown":
      return `parent session '${request.parent_session_id ?? ""}' is unknown to the Prime adapter; no synthetic parent is minted`;
    case "duplicate_session_id":
      return `session '${request.child_session_id}' already exists in the Prime adapter; identity is adapter-owned`;
    case "depth_exceeded":
      return `nested delegation would exceed the workbench recursion policy depth (${request.parent_session_id === null ? "root" : "nested"} child)`;
    case "child_budget_exhausted":
      return `parent '${request.parent_session_id ?? "root"}' already holds the policy maximum of admitted children`;
    case "outcome_not_unknown":
      return "the canonical drive has no unknown outcome; recovery is not applicable";
  }
}

/** FocusRef helper for delegation targets (kept local so callers stay typed). */
export function delegationTargetFocus(source: string, unitId: string): FocusRef {
  return FocusRef.for(source, "WorkItem", unitId);
}
